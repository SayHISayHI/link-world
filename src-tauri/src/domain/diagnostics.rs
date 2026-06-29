use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalMetricsSnapshot {
    pub app_version: String,
    pub data_dir: String,
    pub database_path: String,
    pub object_store_path: String,
    pub database_health: DatabaseHealth,
    pub object_store_health: ObjectStoreHealth,
    pub jobs: JobMetrics,
    pub models: ModelMetrics,
    pub privacy: DiagnosticsPrivacySummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    pub healthy: bool,
    pub quick_check: String,
    pub foreign_key_violations: i64,
    pub applied_migration_version: Option<i64>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectStoreHealth {
    pub healthy: bool,
    pub size_bytes: Option<u64>,
    pub file_count: u64,
    pub issue: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobMetrics {
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
    pub blocked: i64,
    pub cancelled: i64,
    pub recent_failures: Vec<FailedJobSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedJobSummary {
    pub job_id: String,
    pub job_type: String,
    pub status: String,
    pub object_id: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetrics {
    pub configured_count: i64,
    pub enabled_count: i64,
    pub default_chat_configured: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPrivacySummary {
    pub support_bundle_available: bool,
    pub redaction: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportBundleSummary {
    pub bundle_id: String,
    pub created_at: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub included_sections: Vec<String>,
    pub redaction: Vec<String>,
}
