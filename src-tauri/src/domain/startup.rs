use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub mode: StartupMode,
    pub backend_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<StartupIssue>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    Ready,
    Recovery,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupIssue {
    pub code: String,
    pub title: String,
    pub message: String,
    pub recovery_kind: StartupRecoveryKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_backup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration: Option<StartupMigrationRecovery>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartupRecoveryKind {
    DatabaseMigration,
    Restore,
    Database,
    Storage,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupMigrationRecovery {
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_version: Option<i64>,
    pub target_version: i64,
    pub app_version: String,
    pub created_at: String,
}
