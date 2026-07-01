use crate::domain::evaluation::{
    EvaluationArtifactSubmission, EvaluationFailureSubmission, EvaluationInput,
    EvaluationOperation, EvaluationRetryCandidate, EvaluationRunReservation,
    EvaluationRunSubmission, EvaluationTraceCompletion, EVALUATION_TRACE_SCHEMA_VERSION,
};
use crate::domain::knowledge::{EvaluationArtifact, EvaluationRun, EvaluationTrace};
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

    pub async fn find_operation(&self, request_id: &str) -> AppResult<Option<EvaluationOperation>> {
        let row = sqlx::query(
            r#"
            SELECT
                runs.id AS run_id,
                runs.request_id,
                runs.correlation_id,
                runs.retry_of_run_id,
                runs.object_id,
                runs.evaluator_type,
                runs.evaluator_version,
                runs.status,
                jobs.id AS job_id,
                jobs.payload_json
            FROM evaluation_runs AS runs
            INNER JOIN background_jobs AS jobs
                ON jobs.id = runs.request_id
               AND jobs.job_type = 'evaluation.run'
            WHERE runs.request_id = ?1
            "#,
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let payload = row
            .try_get::<Option<String>, _>("payload_json")?
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .ok_or_else(|| {
                AppError::Database("evaluation operation payload is invalid".to_string())
            })?;
        let requested_evaluator_type = payload
            .get("requestedEvaluatorType")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::Database(
                    "evaluation operation is missing requested evaluator identity".to_string(),
                )
            })?;

        Ok(Some(EvaluationOperation {
            run_id: row.get("run_id"),
            request_id: row.get("request_id"),
            correlation_id: row.get("correlation_id"),
            job_id: row.get("job_id"),
            object_id: row.get("object_id"),
            retry_of_run_id: row.get("retry_of_run_id"),
            requested_evaluator_type: requested_evaluator_type.to_string(),
            evaluator_type: row.get("evaluator_type"),
            evaluator_version: row.get("evaluator_version"),
            status: row.get("status"),
        }))
    }

    pub async fn get_retry_candidate(&self, run_id: &str) -> AppResult<EvaluationRetryCandidate> {
        let row = sqlx::query(
            r#"
            SELECT
                runs.id AS run_id,
                runs.object_id,
                runs.status,
                jobs.payload_json
            FROM evaluation_runs AS runs
            INNER JOIN background_jobs AS jobs
                ON jobs.id = runs.request_id
               AND jobs.job_type = 'evaluation.run'
            WHERE runs.id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Err(AppError::ObjectNotFound);
        };
        let payload = row
            .try_get::<Option<String>, _>("payload_json")?
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .ok_or_else(|| AppError::Database("evaluation retry payload is invalid".to_string()))?;
        let requested_evaluator_type = payload
            .get("requestedEvaluatorType")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::Database(
                    "evaluation retry is missing requested evaluator identity".to_string(),
                )
            })?;

        Ok(EvaluationRetryCandidate {
            run_id: row.get("run_id"),
            object_id: row.get("object_id"),
            requested_evaluator_type: requested_evaluator_type.to_string(),
            status: row.get("status"),
        })
    }

    pub async fn reserve_evaluation(
        &self,
        user_id: &str,
        reservation: &EvaluationRunReservation,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO evaluation_runs (
                id,
                request_id,
                correlation_id,
                object_id,
                evaluator_type,
                evaluator_version,
                plan_schema_version,
                input_schema_version,
                output_schema_version,
                status,
                plan_json,
                input_json,
                dimensions_json,
                evidence_json,
                limitations_json,
                next_actions_json,
                verdict,
                retry_of_run_id,
                created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                'planned', ?10, ?11, '{}', '[]', '[]', '[]', 'unknown', ?12, ?13
            )
            "#,
        )
        .bind(&reservation.id)
        .bind(&reservation.request_id)
        .bind(&reservation.correlation_id)
        .bind(&reservation.object_id)
        .bind(&reservation.evaluator_type)
        .bind(&reservation.evaluator_version)
        .bind(reservation.plan_schema_version)
        .bind(reservation.input_schema_version)
        .bind(reservation.output_schema_version)
        .bind(&reservation.plan_json)
        .bind(&reservation.input_json)
        .bind(&reservation.retry_of_run_id)
        .bind(&reservation.created_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO evaluation_traces (
                id, schema_version, evaluation_run_id, request_id, correlation_id, object_id,
                evaluator_type, evaluator_version, execution_kind, input_hash, timeout_ms,
                status, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'planned', ?12)
            "#,
        )
        .bind(&reservation.trace_id)
        .bind(EVALUATION_TRACE_SCHEMA_VERSION)
        .bind(&reservation.id)
        .bind(&reservation.request_id)
        .bind(&reservation.correlation_id)
        .bind(&reservation.object_id)
        .bind(&reservation.evaluator_type)
        .bind(&reservation.evaluator_version)
        .bind(&reservation.execution_kind)
        .bind(&reservation.input_hash)
        .bind(reservation.timeout_ms)
        .bind(&reservation.created_at)
        .execute(&mut *tx)
        .await?;

        let job_payload = json!({
            "schemaVersion": 1,
            "requestId": reservation.request_id,
            "correlationId": reservation.correlation_id,
            "runId": reservation.id,
            "objectId": reservation.object_id,
            "requestedEvaluatorType": reservation.requested_evaluator_type,
            "evaluatorType": reservation.evaluator_type,
            "evaluatorVersion": reservation.evaluator_version,
            "planSchemaVersion": reservation.plan_schema_version,
            "inputSchemaVersion": reservation.input_schema_version,
            "outputSchemaVersion": reservation.output_schema_version,
            "retryOfRunId": reservation.retry_of_run_id,
        })
        .to_string();
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id, job_type, status, object_id, payload_json,
                attempt_count, max_attempts, created_at, updated_at
            ) VALUES (?1, 'evaluation.run', 'queued', ?2, ?3, 0, 1, ?4, ?4)
            "#,
        )
        .bind(&reservation.job_id)
        .bind(&reservation.object_id)
        .bind(job_payload)
        .bind(&reservation.created_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO domain_events (
                id, event_type, event_version, user_id, object_id,
                causation_id, correlation_id, payload_json, occurred_at
            ) VALUES (?1, 'evaluation.planned', 1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(&reservation.object_id)
        .bind(&reservation.job_id)
        .bind(&reservation.correlation_id)
        .bind(
            json!({
                "runId": reservation.id,
                "evaluatorType": reservation.evaluator_type,
                "evaluatorVersion": reservation.evaluator_version,
                "status": "planned",
                "retryOfRunId": reservation.retry_of_run_id,
            })
            .to_string(),
        )
        .bind(&reservation.created_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_evaluation_running(
        &self,
        run_id: &str,
        job_id: &str,
        correlation_id: &str,
        started_at: &str,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let run_update = sqlx::query(
            r#"
            UPDATE evaluation_runs
            SET status = 'running'
            WHERE id = ?1
              AND request_id = ?2
              AND correlation_id = ?3
              AND status = 'planned'
            "#,
        )
        .bind(run_id)
        .bind(job_id)
        .bind(correlation_id)
        .execute(&mut *tx)
        .await?;
        let job_update = sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'running',
                attempt_count = 1,
                locked_at = ?2,
                locked_by = 'local-evaluation-runner',
                updated_at = ?2
            WHERE id = ?1
              AND job_type = 'evaluation.run'
              AND status = 'queued'
            "#,
        )
        .bind(job_id)
        .bind(started_at)
        .execute(&mut *tx)
        .await?;
        let trace_update = sqlx::query(
            r#"
            UPDATE evaluation_traces
            SET status = 'running',
                started_at = ?4
            WHERE evaluation_run_id = ?1
              AND request_id = ?2
              AND correlation_id = ?3
              AND status = 'planned'
            "#,
        )
        .bind(run_id)
        .bind(job_id)
        .bind(correlation_id)
        .bind(started_at)
        .execute(&mut *tx)
        .await?;
        if run_update.rows_affected() != 1
            || job_update.rows_affected() != 1
            || trace_update.rows_affected() != 1
        {
            return Err(AppError::Database(
                "evaluation running transition was rejected".to_string(),
            ));
        }
        tx.commit().await?;
        Ok(())
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

    pub async fn complete_evaluation(
        &self,
        user_id: &str,
        job_id: &str,
        run: &EvaluationRunSubmission,
        artifacts: &[EvaluationArtifactSubmission],
        trace: &EvaluationTraceCompletion,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        let run_update = sqlx::query(
            r#"
            UPDATE evaluation_runs
            SET status = ?4,
                output_json = ?5,
                dimensions_json = ?6,
                evidence_json = ?7,
                limitations_json = ?8,
                next_actions_json = ?9,
                score = ?10,
                verdict = ?11,
                failure_reason = ?12,
                completed_at = ?13
            WHERE id = ?1
              AND request_id = ?2
              AND correlation_id = ?3
              AND evaluator_type = ?14
              AND evaluator_version = ?15
              AND status = 'running'
            "#,
        )
        .bind(&run.id)
        .bind(&run.request_id)
        .bind(&run.correlation_id)
        .bind(&run.status)
        .bind(&run.output_json)
        .bind(&run.dimensions_json)
        .bind(&run.evidence_json)
        .bind(&run.limitations_json)
        .bind(&run.next_actions_json)
        .bind(run.score)
        .bind(&run.verdict)
        .bind(&run.failure_reason)
        .bind(&run.completed_at)
        .bind(&run.evaluator_type)
        .bind(&run.evaluator_version)
        .execute(&mut *tx)
        .await?;
        if run_update.rows_affected() != 1 {
            return Err(AppError::Database(
                "evaluation completion transition was rejected".to_string(),
            ));
        }

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

        let job_update = sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'succeeded',
                last_error = NULL,
                locked_at = NULL,
                locked_by = NULL,
                updated_at = ?2
            WHERE id = ?1
              AND job_type = 'evaluation.run'
              AND status = 'running'
            "#,
        )
        .bind(job_id)
        .bind(&run.completed_at)
        .execute(&mut *tx)
        .await?;
        if job_update.rows_affected() != 1 {
            return Err(AppError::Database(
                "evaluation job completion transition was rejected".to_string(),
            ));
        }

        let trace_update = sqlx::query(
            r#"
            UPDATE evaluation_traces
            SET status = 'passed',
                output_hash = ?4,
                latency_ms = ?5,
                error_code = NULL,
                completed_at = ?6
            WHERE evaluation_run_id = ?1
              AND request_id = ?2
              AND correlation_id = ?3
              AND status = 'running'
            "#,
        )
        .bind(&run.id)
        .bind(&run.request_id)
        .bind(&run.correlation_id)
        .bind(&trace.output_hash)
        .bind(trace.latency_ms)
        .bind(&trace.completed_at)
        .execute(&mut *tx)
        .await?;
        if trace_update.rows_affected() != 1 {
            return Err(AppError::Database(
                "evaluation trace completion transition was rejected".to_string(),
            ));
        }

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
                id, event_type, event_version, user_id, object_id,
                causation_id, correlation_id, payload_json, occurred_at
            ) VALUES (?1, 'evaluation.completed', 1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(&run.object_id)
        .bind(job_id)
        .bind(&run.correlation_id)
        .bind(
            json!({
                "runId": run.id,
                "evaluatorType": run.evaluator_type,
                "evaluatorVersion": run.evaluator_version,
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

    pub async fn fail_evaluation(&self, failure: &EvaluationFailureSubmission) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let run_update = sqlx::query(
            r#"
            UPDATE evaluation_runs
            SET status = 'failed',
                verdict = 'unknown',
                failure_reason = ?4,
                completed_at = ?5
            WHERE id = ?1
              AND request_id = ?2
              AND correlation_id = ?3
              AND status IN ('planned', 'running')
            "#,
        )
        .bind(&failure.run_id)
        .bind(&failure.job_id)
        .bind(&failure.correlation_id)
        .bind(&failure.error_code)
        .bind(&failure.completed_at)
        .execute(&mut *tx)
        .await?;
        let job_update = sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'failed',
                last_error = ?2,
                locked_at = NULL,
                locked_by = NULL,
                updated_at = ?3
            WHERE id = ?1
              AND job_type = 'evaluation.run'
              AND status IN ('queued', 'running')
            "#,
        )
        .bind(&failure.job_id)
        .bind(&failure.error_code)
        .bind(&failure.completed_at)
        .execute(&mut *tx)
        .await?;
        let trace_update = sqlx::query(
            r#"
            UPDATE evaluation_traces
            SET status = 'failed',
                latency_ms = ?4,
                error_code = ?5,
                completed_at = ?6
            WHERE evaluation_run_id = ?1
              AND request_id = ?2
              AND correlation_id = ?3
              AND status IN ('planned', 'running')
            "#,
        )
        .bind(&failure.run_id)
        .bind(&failure.job_id)
        .bind(&failure.correlation_id)
        .bind(failure.latency_ms)
        .bind(&failure.error_code)
        .bind(&failure.completed_at)
        .execute(&mut *tx)
        .await?;
        if run_update.rows_affected() != 1
            || job_update.rows_affected() != 1
            || trace_update.rows_affected() != 1
        {
            return Err(AppError::Database(
                "evaluation failure transition was rejected".to_string(),
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO domain_events (
                id, event_type, event_version, user_id, object_id,
                causation_id, correlation_id, payload_json, occurred_at
            ) VALUES (?1, 'evaluation.failed', 1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&failure.user_id)
        .bind(&failure.object_id)
        .bind(&failure.job_id)
        .bind(&failure.correlation_id)
        .bind(
            json!({
                "runId": failure.run_id,
                "evaluatorType": failure.evaluator_type,
                "status": "failed",
                "errorCode": failure.error_code,
            })
            .to_string(),
        )
        .bind(&failure.completed_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
    pub async fn get_evaluation_run(&self, run_id: &str) -> AppResult<EvaluationRun> {
        let row = sqlx::query(
            r#"

            SELECT
                runs.id,
                runs.request_id,
                runs.correlation_id,
                runs.retry_of_run_id,
                runs.object_id,
                runs.evaluator_type,
                runs.evaluator_version,
                runs.plan_schema_version,
                runs.input_schema_version,
                runs.output_schema_version,
                runs.status,
                runs.dimensions_json,
                runs.evidence_json,
                runs.limitations_json,
                runs.next_actions_json,
                runs.score,
                runs.verdict,
                runs.failure_reason,
                runs.created_at,
                runs.completed_at,
                traces.id AS trace_id,
                traces.schema_version AS trace_schema_version,
                traces.request_id AS trace_request_id,
                traces.correlation_id AS trace_correlation_id,
                traces.evaluator_type AS trace_evaluator_type,
                traces.evaluator_version AS trace_evaluator_version,
                traces.execution_kind AS trace_execution_kind,
                traces.input_hash AS trace_input_hash,
                traces.output_hash AS trace_output_hash,
                traces.timeout_ms AS trace_timeout_ms,
                traces.latency_ms AS trace_latency_ms,
                traces.status AS trace_status,
                traces.error_code AS trace_error_code,
                traces.started_at AS trace_started_at,
                traces.completed_at AS trace_completed_at
            FROM evaluation_runs AS runs
            LEFT JOIN evaluation_traces AS traces ON traces.evaluation_run_id = runs.id
            WHERE runs.id = ?1            "#,
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
    let trace = row
        .try_get::<Option<String>, _>("trace_id")
        .ok()
        .flatten()
        .map(|id| EvaluationTrace {
            id,
            schema_version: row.get("trace_schema_version"),
            request_id: row.get("trace_request_id"),
            correlation_id: row.get("trace_correlation_id"),
            evaluator_type: row.get("trace_evaluator_type"),
            evaluator_version: row.get("trace_evaluator_version"),
            execution_kind: row.get("trace_execution_kind"),
            input_hash: row.get("trace_input_hash"),
            output_hash: row.get("trace_output_hash"),
            timeout_ms: row.get("trace_timeout_ms"),
            latency_ms: row.get("trace_latency_ms"),
            status: row.get("trace_status"),
            error_code: row.get("trace_error_code"),
            started_at: row.get("trace_started_at"),
            completed_at: row.get("trace_completed_at"),
        });

    EvaluationRun {
        id: row.get("id"),
        request_id: row.get("request_id"),
        correlation_id: row.get("correlation_id"),
        retry_of_run_id: row.get("retry_of_run_id"),
        object_id: row.get("object_id"),
        evaluator_type: row.get("evaluator_type"),
        evaluator_version: row.get("evaluator_version"),
        plan_schema_version: row.get("plan_schema_version"),
        input_schema_version: row.get("input_schema_version"),
        output_schema_version: row.get("output_schema_version"),
        status: row.get("status"),
        score: row.get("score"),
        verdict: row.get("verdict"),
        dimensions: parse_json_value(row.get("dimensions_json"))
            .unwrap_or_else(|| Value::Object(Default::default())),
        evidence: parse_json_array(row.get("evidence_json")),
        artifacts,
        trace,
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
