use crate::domain::evaluation::{
    EvaluationArtifactSubmission, EvaluationInput, EvaluationRunSubmission,
};
use crate::domain::knowledge::{EvaluationArtifact, EvaluationRun};
use crate::errors::{AppError, AppResult};
use serde_json::{json, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone)]
pub struct EvaluationRepository {
    pool: SqlitePool,
}

impl EvaluationRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_evaluation_input(&self, object_id: &str) -> AppResult<EvaluationInput> {
        let row = sqlx::query(
            r#"
            SELECT
                objects.id AS object_id,
                objects.user_id,
                objects.object_type,
                objects.title,
                objects.canonical_url,
                objects.privacy_level,
                parsed.id AS parsed_document_id,
                parsed.text_content,
                parsed.word_count,
                parsed.content_hash,
                (
                    SELECT analysis.summary
                    FROM ai_analysis AS analysis
                    WHERE analysis.object_id = objects.id
                    ORDER BY analysis.created_at DESC
                    LIMIT 1
                ) AS latest_ai_summary
            FROM knowledge_objects AS objects
            LEFT JOIN parsed_documents AS parsed ON parsed.id = (
                SELECT latest.id
                FROM parsed_documents AS latest
                WHERE latest.object_id = objects.id
                ORDER BY latest.created_at DESC
                LIMIT 1
            )
            WHERE objects.id = ?1
              AND objects.lifecycle_status != 'deleted'
            "#,
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(AppError::ObjectNotFound);
        };

        let parsed_document_id = row
            .try_get::<Option<String>, _>("parsed_document_id")?
            .ok_or_else(|| {
                AppError::PolicyDenied("object must be parsed before evaluation".to_string())
            })?;

        Ok(EvaluationInput {
            object_id: row.get("object_id"),
            user_id: row.get("user_id"),
            object_type: row.get("object_type"),
            title: row.get("title"),
            canonical_url: row.get("canonical_url"),
            privacy_level: row.get("privacy_level"),
            parsed_document_id,
            text_content: row.get("text_content"),
            word_count: row.get("word_count"),
            content_hash: row.get("content_hash"),
            latest_ai_summary: row.get("latest_ai_summary"),
        })
    }

    pub async fn insert_completed_evaluation(
        &self,
        user_id: &str,
        job_id: &str,
        run: &EvaluationRunSubmission,
        artifacts: &[EvaluationArtifactSubmission],
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO evaluation_runs (
                id,
                object_id,
                evaluator_type,
                evaluator_version,
                status,
                plan_json,
                input_json,
                output_json,
                dimensions_json,
                evidence_json,
                limitations_json,
                next_actions_json,
                score,
                verdict,
                failure_reason,
                created_at,
                completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
        )
        .bind(&run.id)
        .bind(&run.object_id)
        .bind(&run.evaluator_type)
        .bind(&run.evaluator_version)
        .bind(&run.status)
        .bind(&run.plan_json)
        .bind(&run.input_json)
        .bind(&run.output_json)
        .bind(&run.dimensions_json)
        .bind(&run.evidence_json)
        .bind(&run.limitations_json)
        .bind(&run.next_actions_json)
        .bind(run.score)
        .bind(&run.verdict)
        .bind(&run.failure_reason)
        .bind(&run.created_at)
        .bind(&run.completed_at)
        .execute(&mut *tx)
        .await?;

        for artifact in artifacts {
            sqlx::query(
                r#"
                INSERT INTO evaluation_artifacts (
                    id,
                    evaluation_run_id,
                    artifact_type,
                    storage_uri,
                    content_hash,
                    metadata_json,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(&artifact.id)
            .bind(&artifact.evaluation_run_id)
            .bind(&artifact.artifact_type)
            .bind(&artifact.storage_uri)
            .bind(&artifact.content_hash)
            .bind(&artifact.metadata_json)
            .bind(&artifact.created_at)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id,
                job_type,
                status,
                object_id,
                payload_json,
                attempt_count,
                max_attempts,
                last_error,
                created_at,
                updated_at
            ) VALUES (?1, 'evaluation.run', 'succeeded', ?2, ?3, 1, 1, NULL, ?4, ?4)
            "#,
        )
        .bind(job_id)
        .bind(&run.object_id)
        .bind(
            json!({
                "objectId": run.object_id,
                "runId": run.id,
                "evaluatorType": run.evaluator_type,
            })
            .to_string(),
        )
        .bind(&run.completed_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET lifecycle_status = 'evaluated',
                updated_at = ?2
            WHERE id = ?1
              AND lifecycle_status IN ('parsed', 'enriched', 'evaluated')
            "#,
        )
        .bind(&run.object_id)
        .bind(&run.completed_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO domain_events (
                id,
                event_type,
                event_version,
                user_id,
                object_id,
                causation_id,
                payload_json,
                occurred_at
            ) VALUES (?1, 'evaluation.completed', 1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(&run.object_id)
        .bind(job_id)
        .bind(
            json!({
                "objectId": run.object_id,
                "runId": run.id,
                "evaluatorType": run.evaluator_type,
                "status": run.status,
                "verdict": run.verdict,
                "score": run.score,
            })
            .to_string(),
        )
        .bind(&run.completed_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_evaluation_run(&self, run_id: &str) -> AppResult<EvaluationRun> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                object_id,
                evaluator_type,
                evaluator_version,
                status,
                dimensions_json,
                evidence_json,
                limitations_json,
                next_actions_json,
                score,
                verdict,
                failure_reason,
                created_at,
                completed_at
            FROM evaluation_runs
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(AppError::ObjectNotFound);
        };

        let artifacts = self.list_evaluation_artifacts(run_id).await?;
        Ok(evaluation_run_from_row(row, artifacts))
    }

    async fn list_evaluation_artifacts(
        &self,
        evaluation_run_id: &str,
    ) -> AppResult<Vec<EvaluationArtifact>> {
        let rows = sqlx::query(
            r#"
            SELECT
                artifact_type,
                storage_uri,
                metadata_json
            FROM evaluation_artifacts
            WHERE evaluation_run_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .bind(evaluation_run_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(evaluation_artifact_from_row).collect())
    }
}

fn evaluation_run_from_row(row: SqliteRow, artifacts: Vec<EvaluationArtifact>) -> EvaluationRun {
    EvaluationRun {
        id: row.get("id"),
        object_id: row.get("object_id"),
        evaluator_type: row.get("evaluator_type"),
        evaluator_version: row.get("evaluator_version"),
        status: row.get("status"),
        score: row.get("score"),
        verdict: row.get("verdict"),
        dimensions: parse_json_value(row.get("dimensions_json"))
            .unwrap_or_else(|| Value::Object(Default::default())),
        evidence: parse_json_array(row.get("evidence_json")),
        artifacts,
        limitations: parse_json_array(row.get("limitations_json")),
        next_actions: parse_json_array(row.get("next_actions_json")),
        failure_reason: row.get("failure_reason"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    }
}

fn evaluation_artifact_from_row(row: SqliteRow) -> EvaluationArtifact {
    EvaluationArtifact {
        kind: row.get("artifact_type"),
        uri: row.get("storage_uri"),
        metadata: parse_json_value(row.get("metadata_json")),
    }
}

fn parse_json_value(raw: Option<String>) -> Option<Value> {
    raw.and_then(|value| serde_json::from_str(&value).ok())
}

fn parse_json_array<T>(raw: Option<String>) -> Vec<T>
where
    T: serde::de::DeserializeOwned,
{
    raw.and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}
