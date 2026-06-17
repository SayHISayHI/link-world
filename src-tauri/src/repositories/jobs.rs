use crate::domain::jobs::{BackgroundJob, RetriedBackgroundJob};
use crate::errors::{AppError, AppResult};
use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

const DEFAULT_JOB_LIMIT: i64 = 20;
const MAX_JOB_LIMIT: i64 = 100;
const RETRYABLE_CAPTURE_JOB_TYPE: &str = "capture.fetch_url";

#[derive(Debug, Clone)]
pub struct JobsRepository {
    pool: SqlitePool,
}

impl JobsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_background_job(&self, job_id: &str) -> AppResult<BackgroundJob> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                job_type,
                status,
                object_id,
                attempt_count,
                max_attempts,
                next_run_at,
                last_error,
                created_at,
                updated_at
            FROM background_jobs
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(background_job_from_row)
            .ok_or(AppError::JobNotFound)
    }

    pub async fn list_object_jobs(
        &self,
        object_id: &str,
        limit: Option<i64>,
    ) -> AppResult<Vec<BackgroundJob>> {
        let limit = limit.unwrap_or(DEFAULT_JOB_LIMIT).clamp(1, MAX_JOB_LIMIT);
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                job_type,
                status,
                object_id,
                attempt_count,
                max_attempts,
                next_run_at,
                last_error,
                created_at,
                updated_at
            FROM background_jobs
            WHERE object_id = ?1
            ORDER BY created_at DESC, updated_at DESC
            LIMIT ?2
            "#,
        )
        .bind(object_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(background_job_from_row).collect())
    }

    pub async fn retry_background_job(&self, job_id: &str) -> AppResult<RetriedBackgroundJob> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT
                id,
                job_type,
                status,
                object_id
            FROM background_jobs
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = row else {
            return Err(AppError::JobNotFound);
        };

        let job_type: String = row.get("job_type");
        let status: String = row.get("status");
        let object_id: Option<String> = row.get("object_id");
        let object_id = object_id.ok_or_else(|| {
            AppError::PolicyDenied(
                "cannot retry a job that is not attached to an object".to_string(),
            )
        })?;

        if job_type != RETRYABLE_CAPTURE_JOB_TYPE {
            return Err(AppError::PolicyDenied(format!(
                "retry is not implemented for job type: {job_type}"
            )));
        }

        if !matches!(status.as_str(), "failed" | "cancelled" | "blocked") {
            return Err(AppError::PolicyDenied(format!(
                "only failed, cancelled, or blocked jobs can be retried; current status is {status}"
            )));
        }

        let object_update = sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET
                lifecycle_status = 'captured',
                failure_reason = NULL,
                updated_at = ?2
            WHERE id = ?1 AND lifecycle_status != 'deleted'
            "#,
        )
        .bind(&object_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        if object_update.rows_affected() == 0 {
            return Err(AppError::ObjectNotFound);
        }

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET
                status = 'queued',
                attempt_count = 0,
                next_run_at = NULL,
                locked_at = NULL,
                locked_by = NULL,
                last_error = NULL,
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
            INSERT INTO audit_logs (
                id,
                user_id,
                actor_type,
                actor_id,
                action,
                object_id,
                metadata_json,
                created_at
            )
            SELECT
                ?1,
                user_id,
                'local_user',
                user_id,
                'background_job.retry',
                id,
                ?2,
                ?3
            FROM knowledge_objects
            WHERE id = ?4
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(format!(r#"{{"jobId":"{job_id}","jobType":"{job_type}"}}"#))
        .bind(&now)
        .bind(&object_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(RetriedBackgroundJob {
            id: job_id.to_string(),
            job_type,
        })
    }
}

fn background_job_from_row(row: SqliteRow) -> BackgroundJob {
    BackgroundJob {
        id: row.get("id"),
        job_type: row.get("job_type"),
        status: row.get("status"),
        object_id: row.get("object_id"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
        next_run_at: row.get("next_run_at"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::JobsRepository;
    use crate::domain::knowledge::NewKnowledgeObject;
    use crate::errors::AppError;
    use crate::repositories::knowledge_objects::KnowledgeObjectRepository;
    use crate::storage::database::Database;

    #[tokio::test]
    async fn retry_background_job_resets_failed_capture_job_and_object() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_repository = KnowledgeObjectRepository::new(database.pool().clone());
        let object = object_repository
            .insert(NewKnowledgeObject {
                user_id: "local".to_string(),
                object_type: "article".to_string(),
                title: Some("Retry".to_string()),
                canonical_url: Some("https://example.com/retry".to_string()),
                source_platform: Some("web".to_string()),
                author: None,
                privacy_level: "personal".to_string(),
            })
            .await
            .expect("object insert should succeed");

        sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET lifecycle_status = 'failed', failure_reason = 'parse failed'
            WHERE id = ?1
            "#,
        )
        .bind(&object.id)
        .execute(database.pool())
        .await
        .expect("object failure update should succeed");
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
            ) VALUES (
                'job-retry',
                'capture.fetch_url',
                'failed',
                ?1,
                '{}',
                3,
                3,
                'parse failed',
                '2026-06-17T00:00:00Z',
                '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .bind(&object.id)
        .execute(database.pool())
        .await
        .expect("job insert should succeed");

        let repository = JobsRepository::new(database.pool().clone());
        let retried = repository
            .retry_background_job("job-retry")
            .await
            .expect("retry should succeed");

        assert_eq!(retried.id, "job-retry");
        assert_eq!(retried.job_type, "capture.fetch_url");

        let job = repository
            .get_background_job("job-retry")
            .await
            .expect("job should be readable");
        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&object.id)
                .fetch_one(database.pool())
                .await
                .expect("object status should be readable");
        let failure_reason: Option<String> =
            sqlx::query_scalar("SELECT failure_reason FROM knowledge_objects WHERE id = ?1")
                .bind(&object.id)
                .fetch_one(database.pool())
                .await
                .expect("object failure reason should be readable");
        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE object_id = ?1 AND action = 'background_job.retry'",
        )
        .bind(&object.id)
        .fetch_one(database.pool())
        .await
        .expect("audit count should be readable");

        assert_eq!(job.status, "queued");
        assert_eq!(job.attempt_count, 0);
        assert!(job.last_error.is_none());
        assert_eq!(lifecycle_status, "captured");
        assert!(failure_reason.is_none());
        assert_eq!(audit_count, 1);
    }

    #[tokio::test]
    async fn retry_background_job_rejects_non_failed_job() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                'obj-running', 'local', 'article', 'Running', 'personal', 'captured', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            );
            "#,
        )
        .execute(database.pool())
        .await
        .expect("object insert should succeed");
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id, job_type, status, object_id, payload_json, attempt_count, max_attempts, created_at, updated_at
            ) VALUES (
                'job-running', 'capture.fetch_url', 'running', 'obj-running', '{}', 1, 3, '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("job insert should succeed");

        let repository = JobsRepository::new(database.pool().clone());
        let error = repository
            .retry_background_job("job-running")
            .await
            .expect_err("running job should not be retried");

        assert!(matches!(error, AppError::PolicyDenied(_)));
    }
}
