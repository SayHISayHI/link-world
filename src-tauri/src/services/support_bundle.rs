use crate::domain::diagnostics::{
    DatabaseHealth, JobMetrics, ModelMetrics, ObjectStoreHealth, SupportBundleSummary,
};
use crate::errors::{AppError, AppResult};
use crate::services::system::build_local_metrics_snapshot;
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use crate::telemetry::{StructuredLogEntry, StructuredLogger};
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const SUPPORT_BUNDLES_DIR_NAME: &str = "support-bundles";
const SUPPORT_BUNDLE_SCHEMA_VERSION: i64 = 1;
const RECENT_AUDIT_EVENT_LIMIT: i64 = 50;
const RECENT_DOMAIN_EVENT_LIMIT: i64 = 50;

#[derive(Debug, Clone)]
pub struct SupportBundleService {
    database: Database,
    object_store: ObjectStore,
    data_dir: PathBuf,
    export_root: PathBuf,
    app_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportBundleDocument {
    schema_version: i64,
    bundle_id: String,
    created_at: String,
    app_version: String,
    platform: PlatformSummary,
    database_health: DatabaseHealth,
    object_store_health: SupportObjectStoreHealth,
    jobs: SupportJobMetrics,
    models: ModelMetrics,
    feature_flags: Vec<String>,
    plugins: Vec<PluginDiagnosticSummary>,
    recent_audit_events: Vec<AuditEventSummary>,
    recent_domain_events: Vec<DomainEventSummary>,
    runtime_logs: RuntimeLogSummary,
    redaction: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformSummary {
    os: &'static str,
    architecture: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportObjectStoreHealth {
    healthy: bool,
    size_bytes: Option<u64>,
    file_count: u64,
    issue_code: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportJobMetrics {
    queued: i64,
    running: i64,
    failed: i64,
    blocked: i64,
    cancelled: i64,
    recent_failures: Vec<SupportFailedJobSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportFailedJobSummary {
    job_id: String,
    job_type: String,
    status: String,
    object_id: Option<String>,
    error_code: Option<String>,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginDiagnosticSummary {
    plugin_id_hash: String,
    plugin_kind: String,
    version: String,
    enabled: bool,
    manifest_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditEventSummary {
    action: String,
    actor_type: String,
    object_id: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DomainEventSummary {
    event_type: String,
    object_id: Option<String>,
    correlation_id: String,
    occurred_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeLogSummary {
    status: &'static str,
    entries: Vec<StructuredLogEntry>,
}

impl SupportBundleService {
    pub fn new(
        database: Database,
        object_store: ObjectStore,
        data_dir: impl AsRef<Path>,
        app_version: impl Into<String>,
    ) -> Self {
        let data_dir = data_dir.as_ref().to_path_buf();
        Self {
            database,
            object_store,
            export_root: data_dir.join(SUPPORT_BUNDLES_DIR_NAME),
            data_dir,
            app_version: app_version.into(),
        }
    }

    pub async fn export_support_bundle(&self, confirmed: bool) -> AppResult<SupportBundleSummary> {
        if !confirmed {
            return Err(AppError::PolicyDenied(
                "support bundle export requires explicit user confirmation".to_string(),
            ));
        }

        fs::create_dir_all(&self.export_root)?;
        let created_at = Utc::now().to_rfc3339();
        let bundle_id = format!(
            "support-{}-{}",
            Utc::now().format("%Y%m%d%H%M%S"),
            Uuid::new_v4()
        );
        let final_path = self.export_root.join(format!("{bundle_id}.json"));
        let staging_path = self.export_root.join(format!("{bundle_id}.json.staging"));

        let document = self.build_document(&bundle_id, &created_at).await?;
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|_| AppError::Unknown("support bundle serialization failed".to_string()))?;
        self.validate_serialized_bundle(&bytes)?;

        let write_result = (|| -> AppResult<()> {
            fs::write(&staging_path, &bytes)?;
            fs::rename(&staging_path, &final_path)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&staging_path);
        }
        write_result?;

        Ok(SupportBundleSummary {
            bundle_id,
            created_at,
            file_path: final_path.display().to_string(),
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            included_sections: vec![
                "runtime and schema metadata".to_string(),
                "database and object store health".to_string(),
                "job counts and stable failure codes".to_string(),
                "model capability status without configuration details".to_string(),
                "sanitized plugin manifest fingerprints".to_string(),
                "recent audit events without metadata payloads".to_string(),
                "recent domain event types and correlation ids without payloads".to_string(),
                "recent bounded structured runtime logs".to_string(),
            ],
            redaction: support_bundle_redaction(),
        })
    }

    async fn build_document(
        &self,
        bundle_id: &str,
        created_at: &str,
    ) -> AppResult<SupportBundleDocument> {
        let snapshot = build_local_metrics_snapshot(
            &self.app_version,
            &self.data_dir,
            &self.database,
            &self.object_store,
        )
        .await?;
        let plugins = plugin_summaries(&self.database).await?;
        let recent_audit_events = recent_audit_events(&self.database).await?;
        let recent_domain_events = recent_domain_events(&self.database).await?;
        let (runtime_log_status, runtime_log_entries) =
            match StructuredLogger::read_recent(&self.data_dir, Some(100)).await {
                Ok(entries) => ("available", entries),
                Err(_) => ("unavailable", Vec::new()),
            };

        Ok(SupportBundleDocument {
            schema_version: SUPPORT_BUNDLE_SCHEMA_VERSION,
            bundle_id: bundle_id.to_string(),
            created_at: created_at.to_string(),
            app_version: self.app_version.clone(),
            platform: PlatformSummary {
                os: std::env::consts::OS,
                architecture: std::env::consts::ARCH,
            },
            database_health: snapshot.database_health,
            object_store_health: support_object_store_health(snapshot.object_store_health),
            jobs: support_job_metrics(snapshot.jobs),
            models: snapshot.models,
            feature_flags: Vec::new(),
            plugins,
            recent_audit_events,
            recent_domain_events,
            runtime_logs: RuntimeLogSummary {
                status: runtime_log_status,
                entries: runtime_log_entries,
            },
            redaction: support_bundle_redaction(),
        })
    }

    fn validate_serialized_bundle(&self, bytes: &[u8]) -> AppResult<()> {
        let serialized = std::str::from_utf8(bytes).map_err(|_| {
            AppError::Unknown("support bundle encoding validation failed".to_string())
        })?;
        let sensitive_paths = [
            self.data_dir.as_path(),
            self.database.path(),
            self.object_store.root(),
        ];
        let normalized_serialized = serialized.replace("\\\\", "\\");
        if sensitive_paths.iter().any(|path| {
            path.is_absolute()
                && !path.as_os_str().is_empty()
                && normalized_serialized.contains(path.to_string_lossy().as_ref())
        }) || serialized.contains("keyring:model-provider:")
        {
            return Err(AppError::PolicyDenied(
                "support bundle redaction validation failed".to_string(),
            ));
        }

        Ok(())
    }
}

fn support_object_store_health(health: ObjectStoreHealth) -> SupportObjectStoreHealth {
    SupportObjectStoreHealth {
        healthy: health.healthy,
        size_bytes: health.size_bytes,
        file_count: health.file_count,
        issue_code: health.issue.map(|_| "object_store_unavailable"),
    }
}

fn support_job_metrics(metrics: JobMetrics) -> SupportJobMetrics {
    SupportJobMetrics {
        queued: metrics.queued,
        running: metrics.running,
        failed: metrics.failed,
        blocked: metrics.blocked,
        cancelled: metrics.cancelled,
        recent_failures: metrics
            .recent_failures
            .into_iter()
            .map(|failure| SupportFailedJobSummary {
                job_id: safe_identifier(&failure.job_id),
                job_type: safe_label(&failure.job_type),
                status: safe_label(&failure.status),
                object_id: failure.object_id.as_deref().map(safe_identifier),
                error_code: stable_failure_code(failure.last_error.as_deref()),
                updated_at: safe_timestamp(&failure.updated_at),
            })
            .collect(),
    }
}

async fn plugin_summaries(database: &Database) -> AppResult<Vec<PluginDiagnosticSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, version, plugin_kind, enabled, manifest_json
        FROM plugin_manifests
        ORDER BY plugin_kind ASC, id ASC
        "#,
    )
    .fetch_all(database.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let plugin_id: String = row.get("id");
            let manifest_json: String = row.get("manifest_json");
            PluginDiagnosticSummary {
                plugin_id_hash: sha256_hex(plugin_id.as_bytes()),
                plugin_kind: safe_label(&row.get::<String, _>("plugin_kind")),
                version: safe_label(&row.get::<String, _>("version")),
                enabled: row.get::<i64, _>("enabled") != 0,
                manifest_sha256: sha256_hex(manifest_json.as_bytes()),
            }
        })
        .collect())
}

async fn recent_domain_events(database: &Database) -> AppResult<Vec<DomainEventSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT event_type, object_id, correlation_id, occurred_at
        FROM domain_events
        WHERE correlation_id IS NOT NULL
        ORDER BY occurred_at DESC, id DESC
        LIMIT ?1
        "#,
    )
    .bind(RECENT_DOMAIN_EVENT_LIMIT)
    .fetch_all(database.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DomainEventSummary {
            event_type: safe_label(&row.get::<String, _>("event_type")),
            object_id: row
                .get::<Option<String>, _>("object_id")
                .as_deref()
                .map(safe_identifier),
            correlation_id: safe_identifier(&row.get::<String, _>("correlation_id")),
            occurred_at: safe_timestamp(&row.get::<String, _>("occurred_at")),
        })
        .collect())
}

async fn recent_audit_events(database: &Database) -> AppResult<Vec<AuditEventSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT action, actor_type, object_id, created_at
        FROM audit_logs
        ORDER BY created_at DESC, id DESC
        LIMIT ?1
        "#,
    )
    .bind(RECENT_AUDIT_EVENT_LIMIT)
    .fetch_all(database.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AuditEventSummary {
            action: safe_label(&row.get::<String, _>("action")),
            actor_type: safe_label(&row.get::<String, _>("actor_type")),
            object_id: row
                .get::<Option<String>, _>("object_id")
                .as_deref()
                .map(safe_identifier),
            created_at: safe_timestamp(&row.get::<String, _>("created_at")),
        })
        .collect())
}

fn stable_failure_code(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    let prefix = value.split_once(':').map(|(prefix, _)| prefix)?;
    let valid_prefix = prefix.len() <= 64
        && (prefix.starts_with("capture.")
            || prefix.starts_with("ai.")
            || prefix.starts_with("job.")
            || prefix.starts_with("search."))
        && prefix.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_')
        });

    Some(if valid_prefix {
        prefix.to_string()
    } else {
        "failure.redacted".to_string()
    })
}

fn safe_identifier(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        value.to_string()
    } else {
        format!("sha256:{}", sha256_hex(value.as_bytes()))
    }
}

fn safe_label(value: &str) -> String {
    let trimmed = value.trim();
    if !trimmed.is_empty()
        && trimmed.len() <= 96
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '+')
        })
    {
        trimmed.to_string()
    } else {
        "[redacted]".to_string()
    }
}

fn safe_timestamp(value: &str) -> String {
    if chrono::DateTime::parse_from_rfc3339(value).is_ok() {
        value.to_string()
    } else {
        "[invalid-timestamp]".to_string()
    }
}

fn support_bundle_redaction() -> Vec<String> {
    vec![
        "No source snapshots, parsed documents, AI prompts, embeddings, or object bodies."
            .to_string(),
        "No API keys, tokens, cookies, sessions, or credential references.".to_string(),
        "No local absolute data, database, or object store paths.".to_string(),
        "No URL query or fragment values and no raw failed-job messages.".to_string(),
        "Plugin manifests are represented only by safe metadata and SHA-256 fingerprints."
            .to_string(),
        "Audit metadata and domain event payloads are excluded; runtime logs contain only validated structured fields."
            .to_string(),
    ]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{SupportBundleService, SUPPORT_BUNDLES_DIR_NAME};
    use crate::errors::AppError;
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use crate::telemetry::{StructuredLogEvent, StructuredLogger};
    use std::fs;
    use uuid::Uuid;

    #[tokio::test]
    async fn support_bundle_requires_explicit_confirmation() {
        let data_dir =
            std::env::temp_dir().join(format!("node-tide-support-confirm-{}", Uuid::new_v4()));
        let database = Database::initialize(data_dir.clone())
            .await
            .expect("database should initialize");
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");
        let service = SupportBundleService::new(database.clone(), object_store, &data_dir, "0.1.0");

        let error = service
            .export_support_bundle(false)
            .await
            .expect_err("confirmation should be required");
        assert!(matches!(error, AppError::PolicyDenied(_)));
        assert!(!data_dir.join(SUPPORT_BUNDLES_DIR_NAME).exists());

        drop(service);
        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn support_bundle_exports_only_sanitized_operational_metadata() {
        let data_dir =
            std::env::temp_dir().join(format!("node-tide-support-export-{}", Uuid::new_v4()));
        let database = Database::initialize(data_dir.clone())
            .await
            .expect("database should initialize");
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");

        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status
            ) VALUES (
                'obj-support', 'local-user', 'article', 'PRIVATE_BODY_MARKER',
                'secret', 'failed'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("knowledge object should insert");
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id, job_type, status, object_id, payload_json, attempt_count,
                max_attempts, last_error, created_at, updated_at
            ) VALUES (
                'job-support', 'capture.fetch_url', 'failed', 'obj-support',
                '{"url":"https://example.com/a?token=QUERY_SECRET"}', 1, 3,
                'capture.http_forbidden: cookie=session-secret token=provider-secret
                 keyring:model-provider:hidden PRIVATE_BODY_MARKER',
                '2026-06-29T00:00:00Z', '2026-06-29T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("failed job should insert");
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id, job_type, status, object_id, payload_json, attempt_count,
                max_attempts, last_error, created_at, updated_at
            ) VALUES (
                'job-search-support', 'search.rebuild_index', 'failed', NULL,
                '{"stage":"failed","query":"PRIVATE_SEARCH_QUERY"}', 1, 1,
                'search.rebuild_failed: no such table PRIVATE_SEARCH_ERROR',
                '2026-06-29T00:01:00Z', '2026-06-29T00:01:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("failed search job should insert");
        sqlx::query(
            r#"
            INSERT INTO model_provider_configs (
                id, provider, capabilities_json, secret_ref, enabled
            ) VALUES (
                'provider-support', 'openai', '["chat"]',
                'keyring:model-provider:credential-secret', 1
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("model config should insert");
        sqlx::query(
            r#"
            INSERT INTO plugin_manifests (
                id, name, version, plugin_kind, manifest_json, enabled
            ) VALUES (
                'plugin-support', 'Secret plugin', '1.0.0', 'parser',
                '{"token":"PLUGIN_SECRET_MARKER"}', 1
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("plugin manifest should insert");
        sqlx::query(
            r#"
            INSERT INTO domain_events (
                id, event_type, event_version, user_id, object_id,
                correlation_id, payload_json, occurred_at
            ) VALUES (
                'event-support', 'capture.submitted', 1, 'local-user', 'obj-support',
                'd4b258f0-17cf-4b85-81f1-892ad3f10b27',
                '{"url":"https://example.com/?token=DOMAIN_EVENT_SECRET"}',
                '2026-06-29T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("domain event should insert");
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, user_id, actor_type, action, object_id, metadata_json, created_at
            ) VALUES (
                'audit-support', 'local-user', 'system', 'capture.fetch_url.failed',
                'obj-support', '{"body":"AUDIT_SECRET_MARKER"}',
                '2026-06-29T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("audit event should insert");
        fs::write(
            object_store.root().join("PRIVATE_OBJECT_STORE_MARKER.txt"),
            "PRIVATE_OBJECT_BODY_MARKER",
        )
        .expect("object store marker should write");
        StructuredLogger::new(&data_dir)
            .record(
                StructuredLogEvent::info(
                    "capture",
                    "capture.fetch.failed",
                    "Capture fetch job completed with a stable failure code.",
                )
                .with_correlation_id("d4b258f0-17cf-4b85-81f1-892ad3f10b27")
                .with_object_id("obj-support")
                .with_job_id("job-support")
                .with_error_code("capture.http_forbidden"),
            )
            .await
            .expect("structured log should write");

        let service =
            SupportBundleService::new(database.clone(), object_store.clone(), &data_dir, "0.1.0");
        let summary = service
            .export_support_bundle(true)
            .await
            .expect("support bundle should export");
        let bundle_files = fs::read_dir(data_dir.join(SUPPORT_BUNDLES_DIR_NAME))
            .expect("support bundle directory should be readable")
            .collect::<Result<Vec<_>, _>>()
            .expect("support bundle entries should be readable");
        assert_eq!(bundle_files.len(), 1);
        assert!(bundle_files[0]
            .file_name()
            .to_string_lossy()
            .ends_with(".json"));

        let serialized =
            fs::read_to_string(&summary.file_path).expect("support bundle should be readable");
        let document: serde_json::Value =
            serde_json::from_str(&serialized).expect("support bundle should be valid JSON");

        assert_eq!(document["schemaVersion"], 1);
        assert_eq!(document["appVersion"], "0.1.0");
        assert_eq!(
            document["jobs"]["recentFailures"][0]["errorCode"],
            "search.rebuild_failed"
        );
        assert_eq!(
            document["jobs"]["recentFailures"][1]["errorCode"],
            "capture.http_forbidden"
        );
        assert_eq!(document["runtimeLogs"]["status"], "available");
        assert_eq!(
            document["runtimeLogs"]["entries"][0]["correlationId"],
            "d4b258f0-17cf-4b85-81f1-892ad3f10b27"
        );
        assert_eq!(
            document["recentDomainEvents"][0]["correlationId"],
            "d4b258f0-17cf-4b85-81f1-892ad3f10b27"
        );
        assert_eq!(document["plugins"].as_array().map(Vec::len), Some(1));
        assert_eq!(summary.size_bytes, serialized.len() as u64);
        assert_eq!(summary.sha256.len(), 64);
        let normalized_serialized = serialized.replace("\\\\", "\\");
        for forbidden in [
            "PRIVATE_BODY_MARKER",
            "PRIVATE_OBJECT_BODY_MARKER",
            "PLUGIN_SECRET_MARKER",
            "AUDIT_SECRET_MARKER",
            "DOMAIN_EVENT_SECRET",
            "QUERY_SECRET",
            "PRIVATE_SEARCH_QUERY",
            "PRIVATE_SEARCH_ERROR",
            "no such table",
            "session-secret",
            "provider-secret",
            "credential-secret",
            "keyring:model-provider:",
            data_dir.to_string_lossy().as_ref(),
            database.path().to_string_lossy().as_ref(),
            object_store.root().to_string_lossy().as_ref(),
        ] {
            assert!(
                !normalized_serialized.contains(forbidden),
                "support bundle leaked forbidden marker: {forbidden}"
            );
        }

        drop(service);
        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }
}
