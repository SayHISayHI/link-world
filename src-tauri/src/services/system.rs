use crate::domain::diagnostics::{
    DatabaseHealth, DiagnosticsPrivacySummary, FailedJobSummary, JobMetrics, LocalMetricsSnapshot,
    ModelMetrics, ObjectStoreHealth,
};
use crate::errors::{AppError, AppResult};
use crate::state::AppState;
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use sqlx::Row;
use std::path::Path;

const RECENT_FAILURE_LIMIT: i64 = 10;

pub struct SystemService<'a> {
    state: &'a AppState,
}

impl<'a> SystemService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub fn backend_version(&self) -> String {
        self.state.backend_version().to_string()
    }

    pub async fn local_metrics_snapshot(&self, data_dir: &Path) -> AppResult<LocalMetricsSnapshot> {
        build_local_metrics_snapshot(
            self.state.backend_version(),
            data_dir,
            self.state.database()?,
            self.state.object_store()?,
        )
        .await
    }
}

async fn build_local_metrics_snapshot(
    app_version: &str,
    data_dir: &Path,
    database: &Database,
    object_store: &ObjectStore,
) -> AppResult<LocalMetricsSnapshot> {
    let database_health = database_health(database).await?;
    let object_store_health = object_store_health(object_store.root());
    let jobs = job_metrics(database).await?;
    let models = model_metrics(database).await?;

    Ok(LocalMetricsSnapshot {
        app_version: app_version.to_string(),
        data_dir: data_dir.display().to_string(),
        database_path: database.path().display().to_string(),
        object_store_path: object_store.root().display().to_string(),
        database_health,
        object_store_health,
        jobs,
        models,
        privacy: DiagnosticsPrivacySummary {
            support_bundle_available: false,
            redaction: vec![
                "No source snapshots or parsed document content are included.".to_string(),
                "Failed job errors are sanitized before display.".to_string(),
                "Model credential references and API keys are not returned.".to_string(),
                "Support bundle export is intentionally gated behind a future explicit confirmation flow.".to_string(),
            ],
        },
    })
}

async fn database_health(database: &Database) -> AppResult<DatabaseHealth> {
    let quick_check = sqlx::query_scalar::<_, String>("PRAGMA quick_check")
        .fetch_one(database.pool())
        .await?;
    let foreign_key_violations =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pragma_foreign_key_check")
            .fetch_one(database.pool())
            .await?;
    let applied_migration_version = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1",
    )
    .fetch_one(database.pool())
    .await
    .unwrap_or(None);
    let size_bytes = file_size(database.path());

    Ok(DatabaseHealth {
        healthy: quick_check == "ok" && foreign_key_violations == 0,
        quick_check,
        foreign_key_violations,
        applied_migration_version,
        size_bytes,
    })
}

fn object_store_health(root: &Path) -> ObjectStoreHealth {
    match directory_stats(root) {
        Ok((size_bytes, file_count)) => ObjectStoreHealth {
            healthy: true,
            size_bytes: Some(size_bytes),
            file_count,
            issue: None,
        },
        Err(issue) => ObjectStoreHealth {
            healthy: false,
            size_bytes: None,
            file_count: 0,
            issue: Some(sanitize_diagnostic_text(&issue)),
        },
    }
}

async fn job_metrics(database: &Database) -> AppResult<JobMetrics> {
    let rows = sqlx::query(
        r#"
        SELECT status, COUNT(*) AS count
        FROM background_jobs
        GROUP BY status
        "#,
    )
    .fetch_all(database.pool())
    .await?;

    let mut metrics = JobMetrics {
        queued: 0,
        running: 0,
        failed: 0,
        blocked: 0,
        cancelled: 0,
        recent_failures: Vec::new(),
    };

    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        match status.as_str() {
            "queued" => metrics.queued = count,
            "running" => metrics.running = count,
            "failed" => metrics.failed = count,
            "blocked" => metrics.blocked = count,
            "cancelled" => metrics.cancelled = count,
            _ => {}
        }
    }

    let failure_rows = sqlx::query(
        r#"
        SELECT id, job_type, status, object_id, last_error, updated_at
        FROM background_jobs
        WHERE status IN ('failed', 'blocked')
        ORDER BY updated_at DESC, id DESC
        LIMIT ?1
        "#,
    )
    .bind(RECENT_FAILURE_LIMIT)
    .fetch_all(database.pool())
    .await?;

    metrics.recent_failures = failure_rows
        .into_iter()
        .map(|row| FailedJobSummary {
            job_id: row.get("id"),
            job_type: row.get("job_type"),
            status: row.get("status"),
            object_id: row.get("object_id"),
            last_error: row
                .get::<Option<String>, _>("last_error")
                .map(|value| sanitize_diagnostic_text(&value)),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(metrics)
}

async fn model_metrics(database: &Database) -> AppResult<ModelMetrics> {
    let configured_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM model_provider_configs")
            .fetch_one(database.pool())
            .await?;
    let enabled_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM model_provider_configs WHERE enabled = 1",
    )
    .fetch_one(database.pool())
    .await?;
    let default_chat_configured = default_chat_configured(database).await?;
    let status = if default_chat_configured {
        "configured"
    } else if configured_count == 0 {
        "not_configured_normal_degradation"
    } else {
        "missing_default_chat_config"
    };

    Ok(ModelMetrics {
        configured_count,
        enabled_count,
        default_chat_configured,
        status: status.to_string(),
    })
}

async fn default_chat_configured(database: &Database) -> AppResult<bool> {
    let value_json = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value_json FROM local_settings WHERE key = 'models.default.chat.config_id'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap_or(None);

    let Some(value_json) = value_json else {
        return Ok(false);
    };
    let config_id = serde_json::from_str::<String>(&value_json)
        .map_err(|error| AppError::Database(error.to_string()))?;
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM model_provider_configs
        WHERE id = ?1
          AND enabled = 1
          AND capabilities_json LIKE '%chat%'
        "#,
    )
    .bind(config_id)
    .fetch_one(database.pool())
    .await?;

    Ok(count > 0)
}

fn directory_stats(root: &Path) -> Result<(u64, u64), String> {
    let metadata = std::fs::metadata(root).map_err(|error| error.to_string())?;
    if metadata.is_file() {
        return Ok((metadata.len(), 1));
    }

    let mut size = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            let (nested_size, nested_files) = directory_stats(&path)?;
            size += nested_size;
            files += nested_files;
        } else if metadata.is_file() {
            size += metadata.len();
            files += 1;
        }
    }

    Ok((size, files))
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|metadata| metadata.len())
}

fn sanitize_diagnostic_text(value: &str) -> String {
    redact_credential_refs(&redact_url_query_and_fragment(value))
}

fn redact_url_query_and_fragment(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let mut index = 0;

    while index < value.len() {
        let rest = &value[index..];
        if rest.starts_with("http://") || rest.starts_with("https://") {
            let url_start = index;
            let mut cursor = index;
            let mut redacted_tail = false;
            while cursor < value.len() {
                let character = value[cursor..]
                    .chars()
                    .next()
                    .expect("cursor should be on a char boundary");
                if character == '?' || character == '#' {
                    redacted.push_str(&value[url_start..cursor]);
                    redacted.push_str("[redacted]");
                    redacted_tail = true;
                    cursor += character.len_utf8();
                    while cursor < value.len() {
                        let tail = value[cursor..]
                            .chars()
                            .next()
                            .expect("cursor should be on a char boundary");
                        if tail.is_whitespace() || matches!(tail, ')' | ']' | '}' | '"' | '\'') {
                            break;
                        }
                        cursor += tail.len_utf8();
                    }
                    break;
                }
                if character.is_whitespace() {
                    break;
                }
                cursor += character.len_utf8();
            }

            if !redacted_tail {
                redacted.push_str(&value[url_start..cursor]);
            }
            index = cursor;
        } else {
            let character = value[index..]
                .chars()
                .next()
                .expect("cursor should be on a char boundary");
            redacted.push(character);
            index += character.len_utf8();
        }
    }

    redacted
}

fn redact_credential_refs(value: &str) -> String {
    let marker = "keyring:model-provider:";
    let mut output = String::with_capacity(value.len());
    let mut remainder = value;

    while let Some(position) = remainder.find(marker) {
        output.push_str(&remainder[..position]);
        output.push_str("[credential-reference]");
        let after_marker = &remainder[position + marker.len()..];
        let end = after_marker
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ',' | ';' | ')' | ']' | '}')
            })
            .unwrap_or(after_marker.len());
        remainder = &after_marker[end..];
    }

    output.push_str(remainder);
    output
}

#[cfg(test)]
mod tests {
    use super::{build_local_metrics_snapshot, sanitize_diagnostic_text};
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use uuid::Uuid;

    #[test]
    fn sanitizes_url_query_fragments_and_credential_refs() {
        let sanitized = sanitize_diagnostic_text(
            "fetch failed https://example.com/path?token=secret#frag using keyring:model-provider:abc123",
        );

        assert!(sanitized.contains("https://example.com/path[redacted]"));
        assert!(sanitized.contains("[credential-reference]"));
        assert!(!sanitized.contains("token=secret"));
        assert!(!sanitized.contains("keyring:model-provider:abc123"));
    }

    #[tokio::test]
    async fn local_metrics_snapshot_redacts_failed_job_errors_and_treats_missing_model_as_degraded()
    {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-diagnostics-test-{}", Uuid::new_v4()));
        let database = Database::initialize(data_dir.clone())
            .await
            .expect("database should initialize");
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id, job_type, status, payload_json, attempt_count, max_attempts, last_error, created_at, updated_at
            ) VALUES (
                'job-failed', 'capture.fetch_url', 'failed', '{}', 1, 3,
                'capture failed for https://example.com/a?secret=1 using keyring:model-provider:hidden',
                '2026-06-26T00:00:00Z', '2026-06-26T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("failed job should insert");

        let snapshot = build_local_metrics_snapshot("0.1.0", &data_dir, &database, &object_store)
            .await
            .expect("snapshot should build");

        assert!(snapshot.database_health.healthy);
        assert_eq!(snapshot.jobs.failed, 1);
        assert_eq!(snapshot.models.status, "not_configured_normal_degradation");
        let last_error = snapshot.jobs.recent_failures[0]
            .last_error
            .as_ref()
            .expect("last error should exist");
        assert!(last_error.contains("https://example.com/a[redacted]"));
        assert!(!last_error.contains("secret=1"));
        assert!(!last_error.contains("keyring:model-provider:hidden"));
    }
}
