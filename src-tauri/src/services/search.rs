use crate::domain::organization::LibraryQuery;
use crate::domain::search::{
    RebuildSearchIndexResponse, ReindexObjectResponse, SearchIndexHealthResponse, SearchResult,
    SEARCH_HEALTH_FAILURE_REASON, SEARCH_QUERY_FAILURE_REASON, SEARCH_REBUILD_FAILURE_REASON,
    SEARCH_REINDEX_FAILURE_REASON,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::search::SearchRepository;
use crate::state::AppState;
use crate::telemetry::{StructuredLogEvent, StructuredLogger};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SearchService {
    repository: SearchRepository,
    structured_logger: Option<StructuredLogger>,
}

impl SearchService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: SearchRepository::new(state.database()?.pool().clone()),
            structured_logger: state.structured_logger().cloned(),
        })
    }

    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: SearchRepository::new(pool),
            structured_logger: None,
        }
    }

    #[cfg(test)]
    fn with_structured_logger(mut self, structured_logger: StructuredLogger) -> Self {
        self.structured_logger = Some(structured_logger);
        self
    }

    pub async fn search_hybrid(
        &self,
        query: &str,
        limit: Option<i64>,
        filter_type: Option<String>,
    ) -> AppResult<Vec<SearchResult>> {
        self.repository
            .search_hybrid(query, limit, filter_type)
            .await
            .map_err(sanitize_search_query_error)
    }

    pub async fn search_library(
        &self,
        query: &str,
        limit: Option<i64>,
        library_query: LibraryQuery,
    ) -> AppResult<Vec<SearchResult>> {
        self.repository
            .search_library(query, limit, library_query)
            .await
            .map_err(sanitize_search_query_error)
    }
    pub async fn rebuild_search_index(&self) -> AppResult<RebuildSearchIndexResponse> {
        let job_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let response = match self.repository.start_rebuild_index_job(&job_id, &now).await {
            Ok(response) => response,
            Err(error) => {
                self.record_log(
                    StructuredLogEvent::error(
                        "search",
                        "search.rebuild.submit_failed",
                        "Search index rebuild submission failed.",
                    )
                    .with_correlation_id(&job_id)
                    .with_job_id(&job_id)
                    .with_error_code("search.rebuild_failed"),
                )
                .await;
                return Err(sanitize_search_rebuild_error(error));
            }
        };
        self.record_log(
            StructuredLogEvent::info(
                "search",
                "search.rebuild.submitted",
                "Search index rebuild submitted.",
            )
            .with_correlation_id(&job_id)
            .with_job_id(&job_id),
        )
        .await;
        Ok(response)
    }

    pub async fn run_rebuild_search_index(
        &self,
        job_id: &str,
    ) -> AppResult<RebuildSearchIndexResponse> {
        let current = self
            .repository
            .get_rebuild_index_status(job_id)
            .await
            .map_err(sanitize_search_rebuild_error)?;
        if matches!(
            current.status.as_str(),
            "succeeded" | "failed" | "cancelled"
        ) {
            return Ok(current);
        }

        self.record_log(
            StructuredLogEvent::info(
                "search",
                "search.rebuild.started",
                "Search index rebuild started.",
            )
            .with_correlation_id(job_id)
            .with_job_id(job_id),
        )
        .await;

        match self.repository.run_rebuild_index_job(job_id).await {
            Ok(response) => {
                match response.status.as_str() {
                    "succeeded" => {
                        self.record_log(
                            StructuredLogEvent::info(
                                "search",
                                "search.rebuild.succeeded",
                                "Search index rebuild succeeded.",
                            )
                            .with_correlation_id(job_id)
                            .with_job_id(job_id),
                        )
                        .await;
                    }

                    "failed" => {
                        self.record_log(
                            StructuredLogEvent::error(
                                "search",
                                "search.rebuild.failed",
                                "Search index rebuild failed.",
                            )
                            .with_correlation_id(job_id)
                            .with_job_id(job_id)
                            .with_error_code("search.rebuild_failed"),
                        )
                        .await;
                    }
                    _ => {}
                }
                Ok(response)
            }
            Err(error) => {
                self.record_log(
                    StructuredLogEvent::error(
                        "search",
                        "search.rebuild.failed",
                        "Search index rebuild failed.",
                    )
                    .with_correlation_id(job_id)
                    .with_job_id(job_id)
                    .with_error_code("search.rebuild_failed"),
                )
                .await;
                Err(sanitize_search_rebuild_error(error))
            }
        }
    }

    pub async fn get_rebuild_search_index_status(
        &self,
        job_id: &str,
    ) -> AppResult<RebuildSearchIndexResponse> {
        self.repository
            .get_rebuild_index_status(job_id)
            .await
            .map_err(sanitize_search_rebuild_error)
    }

    pub async fn cancel_rebuild_search_index(
        &self,
        job_id: &str,
    ) -> AppResult<RebuildSearchIndexResponse> {
        let before = self
            .repository
            .get_rebuild_index_status(job_id)
            .await
            .map_err(sanitize_search_rebuild_error)?;
        let now = Utc::now().to_rfc3339();
        let response = self
            .repository
            .cancel_rebuild_index_job(job_id, &now)
            .await
            .map_err(sanitize_search_rebuild_error)?;
        if before.status != "cancelled" && response.status == "cancelled" {
            self.record_log(
                StructuredLogEvent::info(
                    "search",
                    "search.rebuild.cancelled",
                    "Search index rebuild cancelled.",
                )
                .with_correlation_id(job_id)
                .with_job_id(job_id),
            )
            .await;
        }
        Ok(response)
    }

    pub async fn check_search_index(&self) -> AppResult<SearchIndexHealthResponse> {
        self.repository
            .check_index_health()
            .await
            .map_err(|_| AppError::Unknown(SEARCH_HEALTH_FAILURE_REASON.to_string()))
    }

    pub async fn reindex_object(&self, object_id: &str) -> AppResult<ReindexObjectResponse> {
        let job_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        match self
            .repository
            .reindex_object_with_job(object_id, &job_id, &now)
            .await
        {
            Ok(indexed) => {
                self.record_log(
                    StructuredLogEvent::info(
                        "search",
                        "search.reindex.succeeded",
                        "Search object reindex succeeded.",
                    )
                    .with_correlation_id(&job_id)
                    .with_object_id(object_id)
                    .with_job_id(&job_id),
                )
                .await;
                Ok(ReindexObjectResponse {
                    job_id,
                    object_id: object_id.to_string(),
                    indexed,
                })
            }
            Err(error) => {
                self.record_log(
                    StructuredLogEvent::error(
                        "search",
                        "search.reindex.failed",
                        "Search object reindex failed.",
                    )
                    .with_correlation_id(&job_id)
                    .with_object_id(object_id)
                    .with_job_id(&job_id)
                    .with_error_code("search.reindex_failed"),
                )
                .await;
                Err(sanitize_search_reindex_error(error))
            }
        }
    }

    async fn record_log(&self, event: StructuredLogEvent) {
        if let Some(logger) = &self.structured_logger {
            let _ = logger.record(event).await;
        }
    }
}

fn sanitize_search_query_error(error: AppError) -> AppError {
    match error {
        AppError::ParseFailed(_) => AppError::ParseFailed(SEARCH_QUERY_FAILURE_REASON.to_string()),
        _ => AppError::Unknown(SEARCH_QUERY_FAILURE_REASON.to_string()),
    }
}

fn sanitize_search_rebuild_error(error: AppError) -> AppError {
    match error {
        AppError::JobNotFound => AppError::JobNotFound,
        AppError::PolicyDenied(message) => AppError::PolicyDenied(message),
        _ => AppError::Unknown(SEARCH_REBUILD_FAILURE_REASON.to_string()),
    }
}

fn sanitize_search_reindex_error(error: AppError) -> AppError {
    match error {
        AppError::ObjectNotFound => AppError::ObjectNotFound,
        AppError::PolicyDenied(message) => AppError::PolicyDenied(message),
        _ => AppError::Unknown(SEARCH_REINDEX_FAILURE_REASON.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        sanitize_search_query_error, sanitize_search_rebuild_error, sanitize_search_reindex_error,
        SearchService,
    };
    use crate::errors::AppError;
    use crate::storage::database::Database;
    use crate::telemetry::StructuredLogger;
    use uuid::Uuid;

    #[test]
    fn search_operation_errors_drop_raw_database_details() {
        let rebuild = sanitize_search_rebuild_error(AppError::Database(
            "no such table PRIVATE_REBUILD_ERROR".to_string(),
        ));
        let reindex = sanitize_search_reindex_error(AppError::Database(
            "disk I/O PRIVATE_REINDEX_ERROR".to_string(),
        ));
        let query = sanitize_search_query_error(AppError::ParseFailed(
            "invalid MATCH PRIVATE_QUERY_ERROR".to_string(),
        ));

        assert!(matches!(
            rebuild,
            AppError::Unknown(message)
                if message == crate::domain::search::SEARCH_REBUILD_FAILURE_REASON
                    && !message.contains("PRIVATE_REBUILD_ERROR")
        ));
        assert!(matches!(
            reindex,
            AppError::Unknown(message)
                if message == crate::domain::search::SEARCH_REINDEX_FAILURE_REASON
                    && !message.contains("PRIVATE_REINDEX_ERROR")
        ));
        assert!(matches!(
            query,
            AppError::ParseFailed(message)
                if message == crate::domain::search::SEARCH_QUERY_FAILURE_REASON
                    && !message.contains("PRIVATE_QUERY_ERROR")
        ));
    }

    #[tokio::test]
    async fn rebuild_logs_persisted_job_id_as_content_free_correlation() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(&database).await;
        let telemetry_dir =
            std::env::temp_dir().join(format!("node-tide-search-rebuild-{}", Uuid::new_v4()));
        let service = SearchService::new(database.pool().clone())
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let submitted = service
            .rebuild_search_index()
            .await
            .expect("rebuild should submit");
        let completed = service
            .run_rebuild_search_index(&submitted.job_id)
            .await
            .expect("rebuild should complete");

        assert!(Uuid::parse_str(&submitted.job_id).is_ok());
        assert_eq!(completed.status, "succeeded");
        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("search rebuild logs should be readable");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec![
                "search.rebuild.submitted",
                "search.rebuild.started",
                "search.rebuild.succeeded",
            ]
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(submitted.job_id.as_str())
                && entry.job_id.as_deref() == Some(submitted.job_id.as_str())
                && entry.object_id.is_none()
                && !entry.message.contains("Synthetic private rebuild body")
        }));

        let persisted_job_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM background_jobs WHERE id = ?1")
                .bind(&submitted.job_id)
                .fetch_one(database.pool())
                .await
                .expect("rebuild job should be readable");
        assert_eq!(persisted_job_count, 1);

        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn rebuild_cancellation_logs_once_and_keeps_the_job_correlation() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let telemetry_dir =
            std::env::temp_dir().join(format!("node-tide-search-cancel-{}", Uuid::new_v4()));
        let service = SearchService::new(database.pool().clone())
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let submitted = service
            .rebuild_search_index()
            .await
            .expect("rebuild should submit");
        let cancelled = service
            .cancel_rebuild_search_index(&submitted.job_id)
            .await
            .expect("rebuild should cancel");
        let repeated = service
            .cancel_rebuild_search_index(&submitted.job_id)
            .await
            .expect("repeated cancellation should be idempotent");

        let converged = service
            .run_rebuild_search_index(&submitted.job_id)
            .await
            .expect("cancelled runner should converge without duplicate logs");

        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(repeated.status, "cancelled");
        assert_eq!(converged.status, "cancelled");
        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("search cancellation logs should be readable");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec!["search.rebuild.submitted", "search.rebuild.cancelled"]
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(submitted.job_id.as_str())
                && entry.job_id.as_deref() == Some(submitted.job_id.as_str())
        }));

        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn object_reindex_logs_success_and_redacted_failure_without_content() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(&database).await;
        let telemetry_dir =
            std::env::temp_dir().join(format!("node-tide-search-reindex-{}", Uuid::new_v4()));
        let service = SearchService::new(database.pool().clone())
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let succeeded = service
            .reindex_object("obj-search-observability")
            .await
            .expect("existing object should reindex");
        let failure = service
            .reindex_object("obj-missing-observability")
            .await
            .expect_err("missing object should fail");

        assert!(succeeded.indexed);
        assert!(Uuid::parse_str(&succeeded.job_id).is_ok());
        assert!(matches!(failure, AppError::ObjectNotFound));
        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("search reindex logs should be readable");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec!["search.reindex.succeeded", "search.reindex.failed"]
        );
        assert_eq!(
            logs.last().and_then(|entry| entry.error_code.as_deref()),
            Some("search.reindex_failed")
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id == entry.job_id
                && !entry.message.contains("Synthetic private rebuild body")
        }));

        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn rebuild_failure_persists_stable_reason_and_logs_only_error_code() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(&database).await;
        let telemetry_dir =
            std::env::temp_dir().join(format!("node-tide-search-failure-{}", Uuid::new_v4()));
        let service = SearchService::new(database.pool().clone())
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let submitted = service
            .rebuild_search_index()
            .await
            .expect("rebuild should submit");
        sqlx::query("DROP TABLE knowledge_fts")
            .execute(database.pool())
            .await
            .expect("fault injection should remove the live FTS table");

        let failed = service
            .run_rebuild_search_index(&submitted.job_id)
            .await
            .expect("runner should converge to a sanitized failed status");

        assert_eq!(failed.status, "failed");
        assert_eq!(
            failed.failure_reason.as_deref(),
            Some(crate::domain::search::SEARCH_REBUILD_FAILURE_REASON)
        );
        assert!(!failed
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("no such table"));
        let staging_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'knowledge_fts_rebuild'",
        )
        .fetch_one(database.pool())
        .await
        .expect("staging table status should be readable");
        assert_eq!(staging_exists, 0);

        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("failed rebuild logs should be readable");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec![
                "search.rebuild.submitted",
                "search.rebuild.started",
                "search.rebuild.failed",
            ]
        );
        assert_eq!(
            logs.last().and_then(|entry| entry.error_code.as_deref()),
            Some("search.rebuild_failed")
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(submitted.job_id.as_str())
                && entry.job_id.as_deref() == Some(submitted.job_id.as_str())
                && !entry.message.contains("no such table")
                && !entry.message.contains("Synthetic private rebuild body")
        }));

        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    async fn seed_searchable_object(database: &Database) {
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status,
                captured_at, updated_at
            ) VALUES (
                'obj-search-observability', 'local', 'article', 'Search observability fixture',
                'personal', 'parsed', '2026-06-30T00:00:00Z', '2026-06-30T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("search fixture object should insert");
        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash,
                parser_id, parser_version, created_at
            ) VALUES (
                'parsed-search-observability', 'obj-search-observability',
                'Search observability fixture',
                'Synthetic private rebuild body that must never enter operational logs.',
                10, 'hash-search-observability', 'test.parser', '0.1.0',
                '2026-06-30T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("search fixture document should insert");
    }
}
