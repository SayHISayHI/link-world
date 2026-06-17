use crate::domain::knowledge::{
    AIAnalysis, AITrace, DeleteObjectMode, DeleteObjectResponse, EvaluationArtifact, EvaluationRun,
    KnowledgeObject, KnowledgeObjectDetail, NewKnowledgeObject, ParsedDocument, SourceSnapshot,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::search::SearchRepository;
use chrono::Utc;
use serde_json::json;
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 100;
const INITIAL_LIFECYCLE_STATUS: &str = "captured";

#[derive(Debug, Clone)]
pub struct KnowledgeObjectRepository {
    pool: SqlitePool,
}

impl KnowledgeObjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_recent(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
        filter_type: Option<String>,
    ) -> AppResult<Vec<KnowledgeObject>> {
        let limit = clamp_limit(limit);
        let offset = offset.unwrap_or(0).max(0);
        let filter_type = normalize_filter(filter_type);

        let rows = sqlx::query(
            r#"
            SELECT
                id,
                user_id,
                object_type,
                title,
                canonical_url,
                source_platform,
                author,
                privacy_level,
                lifecycle_status,
                failure_reason,
                captured_at,
                updated_at
            FROM knowledge_objects
            WHERE lifecycle_status != 'deleted'
              AND (?1 IS NULL OR object_type = ?1)
            ORDER BY updated_at DESC, captured_at DESC
            LIMIT ?2 OFFSET ?3
            "#,
        )
        .bind(filter_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(knowledge_object_from_row).collect())
    }

    pub async fn insert(&self, input: NewKnowledgeObject) -> AppResult<KnowledgeObject> {
        let now = Utc::now().to_rfc3339();
        let object = KnowledgeObject {
            id: Uuid::new_v4().to_string(),
            user_id: input.user_id,
            object_type: input.object_type,
            title: input.title,
            canonical_url: input.canonical_url,
            source_platform: input.source_platform,
            author: input.author,
            privacy_level: input.privacy_level,
            lifecycle_status: INITIAL_LIFECYCLE_STATUS.to_string(),
            failure_reason: None,
            captured_at: now.clone(),
            updated_at: now,
        };

        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id,
                user_id,
                object_type,
                title,
                canonical_url,
                source_platform,
                author,
                privacy_level,
                lifecycle_status,
                failure_reason,
                captured_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(&object.id)
        .bind(&object.user_id)
        .bind(&object.object_type)
        .bind(&object.title)
        .bind(&object.canonical_url)
        .bind(&object.source_platform)
        .bind(&object.author)
        .bind(&object.privacy_level)
        .bind(&object.lifecycle_status)
        .bind(&object.failure_reason)
        .bind(&object.captured_at)
        .bind(&object.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(object)
    }

    pub async fn get_detail(&self, object_id: &str) -> AppResult<KnowledgeObjectDetail> {
        let object = self.get_object(object_id).await?;
        let parsed_document = self.get_latest_parsed_document(object_id).await?;
        let snapshots = self.list_snapshots(object_id).await?;
        let ai_analyses = self.list_ai_analyses(object_id).await?;
        let evaluations = self.list_evaluation_runs(object_id).await?;

        Ok(KnowledgeObjectDetail {
            object,
            parsed_document,
            snapshots,
            ai_analyses,
            evaluations,
        })
    }

    pub async fn delete_object(
        &self,
        object_id: &str,
        mode: DeleteObjectMode,
    ) -> AppResult<DeleteObjectResponse> {
        if matches!(mode, DeleteObjectMode::ExportThenDelete) {
            return Err(AppError::PolicyDenied(
                "export_then_delete is not implemented in the MVP maintenance path".to_string(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            r#"
            SELECT user_id
            FROM knowledge_objects
            WHERE id = ?1 AND lifecycle_status != 'deleted'
            "#,
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Err(AppError::ObjectNotFound);
        };
        let user_id: String = row.get("user_id");
        let tombstone_id = Uuid::new_v4().to_string();
        let event_id = Uuid::new_v4().to_string();
        let audit_id = Uuid::new_v4().to_string();
        let purge_job_id = if matches!(mode, DeleteObjectMode::Purge) {
            Some(Uuid::new_v4().to_string())
        } else {
            None
        };
        let purge_status = if purge_job_id.is_some() {
            "pending"
        } else {
            "completed"
        };
        let completed_at = if purge_job_id.is_some() {
            None
        } else {
            Some(now.clone())
        };

        let mut tx = self.pool.begin().await?;

        let update_result = sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET
                lifecycle_status = 'deleted',
                failure_reason = NULL,
                updated_at = ?2
            WHERE id = ?1 AND lifecycle_status != 'deleted'
            "#,
        )
        .bind(object_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        if update_result.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(AppError::ObjectNotFound);
        }

        SearchRepository::delete_object_index(&mut tx, object_id).await?;

        sqlx::query(
            r#"
            INSERT INTO deletion_tombstones (
                id,
                object_id,
                user_id,
                deletion_mode,
                purge_status,
                reason,
                created_at,
                completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&tombstone_id)
        .bind(object_id)
        .bind(&user_id)
        .bind(mode.as_str())
        .bind(purge_status)
        .bind("user_requested")
        .bind(&now)
        .bind(&completed_at)
        .execute(&mut *tx)
        .await?;

        if let Some(job_id) = &purge_job_id {
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
                    created_at,
                    updated_at
                ) VALUES (?1, 'storage.purge_deleted_object', 'queued', ?2, ?3, 0, 3, ?4, ?4)
                "#,
            )
            .bind(job_id)
            .bind(object_id)
            .bind(
                json!({
                    "objectId": object_id,
                    "tombstoneId": tombstone_id,
                    "deletionMode": mode.as_str(),
                })
                .to_string(),
            )
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO domain_events (
                id,
                event_type,
                event_version,
                user_id,
                object_id,
                payload_json,
                occurred_at
            ) VALUES (?1, 'object.deleted', 1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&event_id)
        .bind(&user_id)
        .bind(object_id)
        .bind(
            json!({
                "objectId": object_id,
                "mode": mode.as_str(),
                "tombstoneId": tombstone_id,
                "purgeJobId": purge_job_id,
            })
            .to_string(),
        )
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id,
                user_id,
                actor_type,
                actor_id,
                action,
                object_id,
                metadata_json,
                created_at
            ) VALUES (?1, ?2, 'local_user', ?2, 'object.delete', ?3, ?4, ?5)
            "#,
        )
        .bind(&audit_id)
        .bind(&user_id)
        .bind(object_id)
        .bind(
            json!({
                "mode": mode.as_str(),
                "tombstoneId": tombstone_id,
                "purgeJobId": purge_job_id,
            })
            .to_string(),
        )
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(DeleteObjectResponse {
            job_id: purge_job_id,
        })
    }

    async fn get_object(&self, object_id: &str) -> AppResult<KnowledgeObject> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                user_id,
                object_type,
                title,
                canonical_url,
                source_platform,
                author,
                privacy_level,
                lifecycle_status,
                failure_reason,
                captured_at,
                updated_at
            FROM knowledge_objects
            WHERE id = ?1 AND lifecycle_status != 'deleted'
            "#,
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(knowledge_object_from_row)
            .ok_or(AppError::ObjectNotFound)
    }

    async fn get_latest_parsed_document(
        &self,
        object_id: &str,
    ) -> AppResult<Option<ParsedDocument>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                object_id,
                source_snapshot_id,
                title,
                text_content,
                markdown_content,
                language,
                word_count,
                content_hash,
                parser_id,
                parser_version,
                created_at
            FROM parsed_documents
            WHERE object_id = ?1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(parsed_document_from_row))
    }

    async fn list_snapshots(&self, object_id: &str) -> AppResult<Vec<SourceSnapshot>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                object_id,
                snapshot_type,
                storage_uri,
                content_hash,
                parser_id,
                parser_version,
                captured_at
            FROM source_snapshots
            WHERE object_id = ?1
            ORDER BY captured_at DESC
            "#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(source_snapshot_from_row).collect())
    }

    async fn list_ai_analyses(&self, object_id: &str) -> AppResult<Vec<AIAnalysis>> {
        let rows = sqlx::query(
            r#"
            SELECT
                analysis.id,
                analysis.object_id,
                analysis.parsed_document_id,
                analysis.analysis_type,
                analysis.schema_version,
                analysis.summary,
                analysis.category,
                analysis.tags_json,
                analysis.key_points_json,
                analysis.claims_json,
                analysis.action_items_json,
                analysis.risks_json,
                analysis.quality_score,
                analysis.confidence,
                analysis.created_at,
                traces.provider AS trace_provider,
                traces.model AS trace_model,
                traces.capability AS trace_capability,
                traces.prompt_template_id AS trace_prompt_template_id,
                traces.prompt_template_version AS trace_prompt_template_version,
                traces.input_snapshot_id AS trace_input_snapshot_id,
                traces.input_parsed_document_id AS trace_input_parsed_document_id,
                traces.input_hash AS trace_input_hash,
                traces.output_hash AS trace_output_hash,
                traces.prompt_tokens AS trace_prompt_tokens,
                traces.completion_tokens AS trace_completion_tokens,
                traces.estimated_cost_usd AS trace_estimated_cost_usd,
                traces.latency_ms AS trace_latency_ms
            FROM ai_analysis AS analysis
            LEFT JOIN ai_traces AS traces ON traces.analysis_id = analysis.id
            WHERE analysis.object_id = ?1
            ORDER BY analysis.created_at DESC
            "#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(ai_analysis_from_row).collect())
    }

    async fn list_evaluation_runs(&self, object_id: &str) -> AppResult<Vec<EvaluationRun>> {
        let rows = sqlx::query(
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
            WHERE object_id = ?1
            ORDER BY created_at DESC
            "#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?;

        let mut evaluations = Vec::with_capacity(rows.len());

        for row in rows {
            let evaluation_id: String = row.get("id");
            let artifacts = self.list_evaluation_artifacts(&evaluation_id).await?;
            evaluations.push(evaluation_run_from_row(row, artifacts));
        }

        Ok(evaluations)
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

fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

fn normalize_filter(filter_type: Option<String>) -> Option<String> {
    filter_type.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn knowledge_object_from_row(row: SqliteRow) -> KnowledgeObject {
    KnowledgeObject {
        id: row.get("id"),
        user_id: row.get("user_id"),
        object_type: row.get("object_type"),
        title: row.get("title"),
        canonical_url: row.get("canonical_url"),
        source_platform: row.get("source_platform"),
        author: row.get("author"),
        privacy_level: row.get("privacy_level"),
        lifecycle_status: row.get("lifecycle_status"),
        failure_reason: row.get("failure_reason"),
        captured_at: row.get("captured_at"),
        updated_at: row.get("updated_at"),
    }
}

fn source_snapshot_from_row(row: SqliteRow) -> SourceSnapshot {
    SourceSnapshot {
        id: row.get("id"),
        object_id: row.get("object_id"),
        snapshot_type: row.get("snapshot_type"),
        storage_uri: row.get("storage_uri"),
        content_hash: row.get("content_hash"),
        parser_id: row.get("parser_id"),
        parser_version: row.get("parser_version"),
        captured_at: row.get("captured_at"),
    }
}

fn parsed_document_from_row(row: SqliteRow) -> ParsedDocument {
    ParsedDocument {
        id: row.get("id"),
        object_id: row.get("object_id"),
        source_snapshot_id: row.get("source_snapshot_id"),
        title: row.get("title"),
        text_content: row.get("text_content"),
        markdown_content: row.get("markdown_content"),
        language: row.get("language"),
        word_count: row.get("word_count"),
        content_hash: row.get("content_hash"),
        parser_id: row.get("parser_id"),
        parser_version: row.get("parser_version"),
        created_at: row.get("created_at"),
    }
}

fn ai_analysis_from_row(row: SqliteRow) -> AIAnalysis {
    let trace = match row.try_get::<Option<String>, _>("trace_provider") {
        Ok(Some(provider)) => Some(AITrace {
            provider,
            model: row.get("trace_model"),
            capability: row.get("trace_capability"),
            prompt_template_id: row.get("trace_prompt_template_id"),
            prompt_template_version: row.get("trace_prompt_template_version"),
            input_snapshot_id: row.get("trace_input_snapshot_id"),
            input_parsed_document_id: row.get("trace_input_parsed_document_id"),
            input_hash: row.get("trace_input_hash"),
            output_hash: row.get("trace_output_hash"),
            prompt_tokens: row.get("trace_prompt_tokens"),
            completion_tokens: row.get("trace_completion_tokens"),
            estimated_cost_usd: row.get("trace_estimated_cost_usd"),
            latency_ms: row.get("trace_latency_ms"),
        }),
        _ => None,
    };

    AIAnalysis {
        id: row.get("id"),
        object_id: row.get("object_id"),
        parsed_document_id: row.get("parsed_document_id"),
        analysis_type: row.get("analysis_type"),
        schema_version: row.get("schema_version"),
        summary: row.get("summary"),
        category: row.get("category"),
        tags: parse_json_array(row.get("tags_json")),
        key_points: parse_json_array(row.get("key_points_json")),
        claims: parse_json_array(row.get("claims_json")),
        action_items: parse_json_array(row.get("action_items_json")),
        risks: parse_json_array(row.get("risks_json")),
        quality_score: row.get("quality_score"),
        confidence: row.get("confidence"),
        trace,
        created_at: row.get("created_at"),
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

#[cfg(test)]
mod tests {
    use super::KnowledgeObjectRepository;
    use crate::domain::knowledge::{DeleteObjectMode, NewKnowledgeObject};
    use crate::errors::AppError;
    use crate::repositories::search::SearchRepository;
    use crate::storage::database::Database;

    #[tokio::test]
    async fn insert_generates_database_owned_fields_and_persists_object() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = KnowledgeObjectRepository::new(database.pool().clone());

        let inserted = repository
            .insert(NewKnowledgeObject {
                user_id: "local".to_string(),
                object_type: "article".to_string(),
                title: Some("A captured article".to_string()),
                canonical_url: Some("https://example.com/article".to_string()),
                source_platform: Some("web".to_string()),
                author: None,
                privacy_level: "personal".to_string(),
            })
            .await
            .expect("insert should succeed");

        assert!(uuid::Uuid::parse_str(&inserted.id).is_ok());
        assert_eq!(inserted.lifecycle_status, "captured");
        assert!(inserted.failure_reason.is_none());
        assert_eq!(inserted.captured_at, inserted.updated_at);

        let objects = repository
            .list_recent(Some(10), Some(0), Some("article".to_string()))
            .await
            .expect("query should succeed");

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, inserted.id);
        assert_eq!(objects[0].title.as_deref(), Some("A captured article"));
    }

    #[tokio::test]
    async fn list_recent_returns_non_deleted_objects_in_updated_order() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");

        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES
              ('obj-old', 'local', 'article', 'Old Article', 'personal', 'captured', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
              ('obj-new', 'local', 'prompt', 'New Prompt', 'personal', 'parsed', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
              ('obj-deleted', 'local', 'article', 'Deleted', 'personal', 'deleted', '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z')
            "#,
        )
        .execute(database.pool())
        .await
        .expect("insert should succeed");

        let repository = KnowledgeObjectRepository::new(database.pool().clone());
        let objects = repository
            .list_recent(Some(10), Some(0), None)
            .await
            .expect("query should succeed");

        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].id, "obj-new");
        assert_eq!(objects[1].id, "obj-old");
    }

    #[tokio::test]
    async fn soft_delete_marks_object_deleted_and_writes_audit_trail() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = KnowledgeObjectRepository::new(database.pool().clone());
        let inserted = repository
            .insert(NewKnowledgeObject {
                user_id: "local".to_string(),
                object_type: "article".to_string(),
                title: Some("Delete Me".to_string()),
                canonical_url: Some("https://example.com/delete-me".to_string()),
                source_platform: Some("web".to_string()),
                author: None,
                privacy_level: "personal".to_string(),
            })
            .await
            .expect("insert should succeed");

        let response = repository
            .delete_object(&inserted.id, DeleteObjectMode::SoftDelete)
            .await
            .expect("soft delete should succeed");

        assert!(response.job_id.is_none());

        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&inserted.id)
                .fetch_one(database.pool())
                .await
                .expect("object status should be readable");
        let tombstone_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM deletion_tombstones WHERE object_id = ?1")
                .bind(&inserted.id)
                .fetch_one(database.pool())
                .await
                .expect("tombstone count should be readable");
        let event_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM domain_events WHERE object_id = ?1 AND event_type = 'object.deleted'",
        )
        .bind(&inserted.id)
        .fetch_one(database.pool())
        .await
        .expect("event count should be readable");
        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE object_id = ?1 AND action = 'object.delete'",
        )
        .bind(&inserted.id)
        .fetch_one(database.pool())
        .await
        .expect("audit count should be readable");

        assert_eq!(lifecycle_status, "deleted");
        assert_eq!(tombstone_count, 1);
        assert_eq!(event_count, 1);
        assert_eq!(audit_count, 1);

        let objects = repository
            .list_recent(Some(10), Some(0), None)
            .await
            .expect("list should succeed");
        assert!(objects.is_empty());

        let detail_error = repository
            .get_detail(&inserted.id)
            .await
            .expect_err("deleted object should not be returned");
        assert!(matches!(detail_error, AppError::ObjectNotFound));
    }

    #[tokio::test]
    async fn purge_delete_queues_storage_cleanup_job() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = KnowledgeObjectRepository::new(database.pool().clone());
        let inserted = repository
            .insert(NewKnowledgeObject {
                user_id: "local".to_string(),
                object_type: "article".to_string(),
                title: Some("Purge Me".to_string()),
                canonical_url: Some("https://example.com/purge-me".to_string()),
                source_platform: Some("web".to_string()),
                author: None,
                privacy_level: "personal".to_string(),
            })
            .await
            .expect("insert should succeed");

        let response = repository
            .delete_object(&inserted.id, DeleteObjectMode::Purge)
            .await
            .expect("purge delete should succeed");
        let job_id = response.job_id.expect("purge should create a job");

        let job_type: String =
            sqlx::query_scalar("SELECT job_type FROM background_jobs WHERE id = ?1")
                .bind(&job_id)
                .fetch_one(database.pool())
                .await
                .expect("job type should be readable");
        let purge_status: String =
            sqlx::query_scalar("SELECT purge_status FROM deletion_tombstones WHERE object_id = ?1")
                .bind(&inserted.id)
                .fetch_one(database.pool())
                .await
                .expect("purge status should be readable");

        assert_eq!(job_type, "storage.purge_deleted_object");
        assert_eq!(purge_status, "pending");
    }

    #[tokio::test]
    async fn soft_delete_removes_object_from_search_index() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = KnowledgeObjectRepository::new(database.pool().clone());
        let inserted = repository
            .insert(NewKnowledgeObject {
                user_id: "local".to_string(),
                object_type: "article".to_string(),
                title: Some("Search Delete".to_string()),
                canonical_url: Some("https://example.com/search-delete".to_string()),
                source_platform: Some("web".to_string()),
                author: None,
                privacy_level: "personal".to_string(),
            })
            .await
            .expect("insert should succeed");

        sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET lifecycle_status = 'parsed'
            WHERE id = ?1
            "#,
        )
        .bind(&inserted.id)
        .execute(database.pool())
        .await
        .expect("object should update");
        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (
                'parsed-delete-search', ?1, 'Search Delete',
                'This object should disappear from the search index after deletion.',
                9, 'hash-delete-search', 'test.parser', '0.1.0', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .bind(&inserted.id)
        .execute(database.pool())
        .await
        .expect("parsed document should insert");

        let mut tx = database.pool().begin().await.expect("tx should begin");
        SearchRepository::reindex_object(&mut tx, &inserted.id)
            .await
            .expect("object should reindex");
        tx.commit().await.expect("tx should commit");

        let search_repository = SearchRepository::new(database.pool().clone());
        let before_delete = search_repository
            .search_hybrid("disappear search index", Some(10))
            .await
            .expect("search should work");
        assert_eq!(before_delete.len(), 1);

        repository
            .delete_object(&inserted.id, DeleteObjectMode::SoftDelete)
            .await
            .expect("soft delete should succeed");

        let after_delete = search_repository
            .search_hybrid("disappear search index", Some(10))
            .await
            .expect("search should work");
        assert!(after_delete.is_empty());
    }
}
