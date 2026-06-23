use crate::domain::ai::{
    AIAnalysisSubmission, AIEnrichmentInput, AITraceSubmission, ModelProviderConfig,
    StoredModelProviderConfig,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::search::SearchRepository;
use chrono::Utc;
use serde_json::json;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

const AI_ENRICHMENT_JOB_TYPE: &str = "ai.enrich_object";

#[derive(Debug, Clone)]
pub struct AIRepository {
    pool: SqlitePool,
}

impl AIRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_model_provider_config(
        &self,
        config: &ModelProviderConfig,
        secret_ref: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let config_id = normalize_provider_id(&config.provider)?;
        let capabilities_json = serde_json::to_string(&config.capabilities).map_err(|error| {
            AppError::ModelOutputSchema(format!("invalid capabilities: {error}"))
        })?;

        sqlx::query(
            r#"
            INSERT INTO model_provider_configs (
                id,
                provider,
                chat_base_url,
                embeddings_base_url,
                default_chat_model,
                default_embedding_model,
                capabilities_json,
                secret_ref,
                enabled,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                chat_base_url = excluded.chat_base_url,
                embeddings_base_url = excluded.embeddings_base_url,
                default_chat_model = excluded.default_chat_model,
                default_embedding_model = excluded.default_embedding_model,
                capabilities_json = excluded.capabilities_json,
                secret_ref = COALESCE(excluded.secret_ref, model_provider_configs.secret_ref),
                enabled = 1,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&config_id)
        .bind(config.provider.trim())
        .bind(config.chat_base_url.as_deref().map(str::trim))
        .bind(config.embeddings_base_url.as_deref().map(str::trim))
        .bind(config.default_chat_model.as_deref().map(str::trim))
        .bind(config.default_embedding_model.as_deref().map(str::trim))
        .bind(capabilities_json)
        .bind(secret_ref)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_enabled_chat_config(&self) -> AppResult<Option<StoredModelProviderConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                provider,
                chat_base_url,
                embeddings_base_url,
                default_chat_model,
                default_embedding_model,
                capabilities_json,
                secret_ref,
                enabled
            FROM model_provider_configs
            WHERE enabled = 1
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(stored_model_config_from_row)
            .find(|config| {
                config
                    .capabilities
                    .iter()
                    .any(|capability| capability == "chat")
            }))
    }

    pub async fn get_enrichment_input(&self, object_id: &str) -> AppResult<AIEnrichmentInput> {
        let row = sqlx::query(
            r#"
            SELECT
                objects.id AS object_id,
                objects.object_type,
                objects.title,
                objects.canonical_url,
                objects.privacy_level,
                parsed.id AS parsed_document_id,
                parsed.source_snapshot_id,
                parsed.text_content,
                parsed.content_hash
            FROM knowledge_objects AS objects
            INNER JOIN parsed_documents AS parsed ON parsed.object_id = objects.id
            WHERE objects.id = ?1
              AND objects.lifecycle_status != 'deleted'
            ORDER BY parsed.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(enrichment_input_from_row)
            .ok_or(AppError::ObjectNotFound)
    }

    pub async fn create_enrichment_job(&self, object_id: &str) -> AppResult<String> {
        let now = Utc::now().to_rfc3339();
        let job_id = Uuid::new_v4().to_string();

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
            ) VALUES (?1, ?2, 'running', ?3, ?4, 1, 1, ?5, ?5)
            "#,
        )
        .bind(&job_id)
        .bind(AI_ENRICHMENT_JOB_TYPE)
        .bind(object_id)
        .bind(json!({ "objectId": object_id }).to_string())
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(job_id)
    }

    pub async fn complete_enrichment_job(
        &self,
        job_id: &str,
        analysis: &AIAnalysisSubmission,
        trace: &AITraceSubmission,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO ai_analysis (
                id,
                object_id,
                parsed_document_id,
                analysis_type,
                schema_version,
                summary,
                category,
                tags_json,
                key_points_json,
                claims_json,
                action_items_json,
                risks_json,
                quality_score,
                confidence,
                display_hints_json,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
        )
        .bind(&analysis.id)
        .bind(&analysis.object_id)
        .bind(&analysis.parsed_document_id)
        .bind(&analysis.analysis_type)
        .bind(analysis.schema_version)
        .bind(&analysis.summary)
        .bind(&analysis.category)
        .bind(&analysis.tags_json)
        .bind(&analysis.key_points_json)
        .bind(&analysis.claims_json)
        .bind(&analysis.action_items_json)
        .bind(&analysis.risks_json)
        .bind(analysis.quality_score)
        .bind(analysis.confidence)
        .bind(&analysis.display_hints_json)
        .bind(&analysis.created_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO ai_traces (
                id,
                analysis_id,
                object_id,
                provider,
                model,
                capability,
                prompt_template_id,
                prompt_template_version,
                input_snapshot_id,
                input_parsed_document_id,
                input_hash,
                output_hash,
                prompt_tokens,
                completion_tokens,
                estimated_cost_usd,
                latency_ms,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
        )
        .bind(&trace.id)
        .bind(&trace.analysis_id)
        .bind(&trace.object_id)
        .bind(&trace.provider)
        .bind(&trace.model)
        .bind(&trace.capability)
        .bind(&trace.prompt_template_id)
        .bind(&trace.prompt_template_version)
        .bind(&trace.input_snapshot_id)
        .bind(&trace.input_parsed_document_id)
        .bind(&trace.input_hash)
        .bind(&trace.output_hash)
        .bind(trace.prompt_tokens)
        .bind(trace.completion_tokens)
        .bind(trace.estimated_cost_usd)
        .bind(trace.latency_ms)
        .bind(&trace.created_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'succeeded',
                last_error = NULL,
                locked_at = NULL,
                locked_by = NULL,
                updated_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET lifecycle_status = 'enriched',
                updated_at = ?2
            WHERE id = ?1 AND lifecycle_status = 'parsed'
            "#,
        )
        .bind(&analysis.object_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        SearchRepository::reindex_object(&mut tx, &analysis.object_id).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn fail_enrichment_job(&self, job_id: &str, failure_reason: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let failure_reason = truncate_failure_reason(failure_reason);

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'failed',
                last_error = ?2,
                locked_at = NULL,
                locked_by = NULL,
                updated_at = ?3
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(failure_reason)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn normalize_provider_id(provider: &str) -> AppResult<String> {
    let provider = provider.trim();
    if provider.is_empty() {
        return Err(AppError::PolicyDenied(
            "model provider is required".to_string(),
        ));
    }

    Ok(provider.to_ascii_lowercase().replace(' ', "_"))
}

fn stored_model_config_from_row(row: SqliteRow) -> StoredModelProviderConfig {
    let capabilities_json: String = row.get("capabilities_json");
    let capabilities = serde_json::from_str::<Vec<String>>(&capabilities_json).unwrap_or_default();

    StoredModelProviderConfig {
        id: row.get("id"),
        provider: row.get("provider"),
        chat_base_url: row.get("chat_base_url"),
        embeddings_base_url: row.get("embeddings_base_url"),
        default_chat_model: row.get("default_chat_model"),
        default_embedding_model: row.get("default_embedding_model"),
        capabilities,
        secret_ref: row.get("secret_ref"),
        enabled: row.get::<i64, _>("enabled") == 1,
    }
}

fn enrichment_input_from_row(row: SqliteRow) -> AIEnrichmentInput {
    AIEnrichmentInput {
        object_id: row.get("object_id"),
        object_type: row.get("object_type"),
        title: row.get("title"),
        canonical_url: row.get("canonical_url"),
        privacy_level: row.get("privacy_level"),
        parsed_document_id: row.get("parsed_document_id"),
        source_snapshot_id: row.get("source_snapshot_id"),
        text_content: row.get("text_content"),
        content_hash: row.get("content_hash"),
    }
}

fn truncate_failure_reason(reason: &str) -> String {
    const MAX_FAILURE_REASON_CHARS: usize = 500;

    if reason.chars().count() <= MAX_FAILURE_REASON_CHARS {
        return reason.to_string();
    }

    reason
        .chars()
        .take(MAX_FAILURE_REASON_CHARS)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::AIRepository;
    use crate::domain::ai::{AIAnalysisSubmission, AITraceSubmission, ModelProviderConfig};
    use crate::repositories::search::SearchRepository;
    use crate::storage::database::Database;

    #[tokio::test]
    async fn upsert_model_provider_config_does_not_store_api_key() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = AIRepository::new(database.pool().clone());

        repository
            .upsert_model_provider_config(
                &ModelProviderConfig {
                    provider: "openai-compatible".to_string(),
                    chat_base_url: Some("https://api.openai.com/v1".to_string()),
                    embeddings_base_url: None,
                    api_key: Some("sk-secret".to_string()),
                    default_chat_model: Some("gpt-4.1-mini".to_string()),
                    default_embedding_model: None,
                    capabilities: vec!["chat".to_string()],
                },
                Some("memory:model_provider:openai-compatible:api_key"),
            )
            .await
            .expect("config should upsert");

        let row: (String, Option<String>) =
            sqlx::query_as("SELECT capabilities_json, secret_ref FROM model_provider_configs")
                .fetch_one(database.pool())
                .await
                .expect("config row should be readable");

        assert_eq!(row.0, "[\"chat\"]");
        assert_eq!(
            row.1.as_deref(),
            Some("memory:model_provider:openai-compatible:api_key")
        );

        let leaked_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM model_provider_configs WHERE secret_ref LIKE '%sk-secret%'",
        )
        .fetch_one(database.pool())
        .await
        .expect("leak query should be readable");

        assert_eq!(leaked_count, 0);
    }

    #[tokio::test]
    async fn complete_enrichment_job_updates_search_index() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = AIRepository::new(database.pool().clone());

        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                'obj-ai-search', 'local', 'article', 'AI Search Article', 'personal', 'parsed',
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("object should insert");
        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (
                'parsed-ai-search', 'obj-ai-search', 'AI Search Article',
                'Original text before AI summary.',
                5, 'hash-ai-search', 'test.parser', '0.1.0', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("parsed document should insert");

        let job_id = repository
            .create_enrichment_job("obj-ai-search")
            .await
            .expect("job should create");
        let analysis = AIAnalysisSubmission {
            id: "analysis-ai-search".to_string(),
            object_id: "obj-ai-search".to_string(),
            parsed_document_id: "parsed-ai-search".to_string(),
            analysis_type: "general_summary".to_string(),
            schema_version: 1,
            summary: "Searchable summary from AI analysis.".to_string(),
            category: Some("engineering".to_string()),
            tags_json: "[]".to_string(),
            key_points_json: "[]".to_string(),
            claims_json: "[]".to_string(),
            action_items_json: "[]".to_string(),
            risks_json: "[]".to_string(),
            quality_score: Some(0.8),
            confidence: Some(0.7),
            display_hints_json: Some(
                r#"{"schemaVersion":1,"mode":"reference","confidence":0.9}"#.to_string(),
            ),
            created_at: "2026-06-17T00:00:01Z".to_string(),
        };
        let trace = AITraceSubmission {
            id: "trace-ai-search".to_string(),
            analysis_id: "analysis-ai-search".to_string(),
            object_id: "obj-ai-search".to_string(),
            provider: "test".to_string(),
            model: "test-model".to_string(),
            capability: "chat".to_string(),
            prompt_template_id: None,
            prompt_template_version: None,
            input_snapshot_id: None,
            input_parsed_document_id: Some("parsed-ai-search".to_string()),
            input_hash: Some("hash-ai-search".to_string()),
            output_hash: Some("hash-output".to_string()),
            prompt_tokens: None,
            completion_tokens: None,
            estimated_cost_usd: None,
            latency_ms: Some(10),
            created_at: "2026-06-17T00:00:01Z".to_string(),
        };

        repository
            .complete_enrichment_job(&job_id, &analysis, &trace)
            .await
            .expect("job should complete");

        let stored_display_hints: Option<String> = sqlx::query_scalar(
            "SELECT display_hints_json FROM ai_analysis WHERE id = 'analysis-ai-search'",
        )
        .fetch_one(database.pool())
        .await
        .expect("display hints should be readable");
        assert_eq!(stored_display_hints, analysis.display_hints_json);

        let search_results = SearchRepository::new(database.pool().clone())
            .search_hybrid("Searchable summary", Some(10))
            .await
            .expect("AI summary should be searchable");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].object.id, "obj-ai-search");
        assert!(search_results[0]
            .matched_fields
            .iter()
            .any(|field| field == "aiSummary"));
    }
}
