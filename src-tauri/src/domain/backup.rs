use serde::{Deserialize, Serialize};

pub const BACKUP_MANIFEST_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileEntry {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub schema_version: i64,
    pub backup_id: String,
    pub app_version: String,
    pub created_at: String,
    pub database: BackupFileEntry,
    pub objects: Vec<BackupFileEntry>,
    pub total_size_bytes: u64,
    pub contains_user_content: bool,
    pub credentials_included: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub backup_id: String,
    pub app_version: Option<String>,
    pub created_at: Option<String>,
    pub object_file_count: usize,
    pub total_size_bytes: u64,
    pub status: String,
}

impl From<&BackupManifest> for BackupSummary {
    fn from(manifest: &BackupManifest) -> Self {
        Self {
            backup_id: manifest.backup_id.clone(),
            app_version: Some(manifest.app_version.clone()),
            created_at: Some(manifest.created_at.clone()),
            object_file_count: manifest.objects.len(),
            total_size_bytes: manifest.total_size_bytes,
            status: "ready".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupVerification {
    pub backup_id: String,
    pub valid: bool,
    pub checked_file_count: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreparation {
    pub correlation_id: String,
    pub backup_id: String,
    pub safety_backup_id: String,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub backup_id: String,
    pub safety_backup_id: String,
    pub status: String,
    pub completed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
