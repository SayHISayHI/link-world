use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCaptureItem {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub source_type: String,
    pub source_platform: Option<String>,
    pub source_url: Option<String>,
    pub canonical_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub captured_at: Option<String>,
    pub raw_html: Option<String>,
    pub raw_text: Option<String>,
    #[serde(default)]
    pub assets: Vec<CaptureAsset>,
    #[serde(default)]
    pub metadata: Value,
    pub privacy_level: String,
    pub permission_context: PermissionContext,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureAsset {
    pub id: String,
    pub kind: String,
    pub mime_type: String,
    pub uri: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionContext {
    pub acquisition_mode: String,
    pub user_confirmed: bool,
    pub platform_terms_hint: Option<String>,
    pub allowed_for_cloud_processing: bool,
    pub allowed_for_third_party_ai: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitCaptureResponse {
    pub object_id: String,
    pub snapshot_id: String,
    pub parsed_document_id: Option<String>,
    pub job_id: String,
}

#[derive(Debug, Clone)]
pub struct CaptureSubmission {
    pub object_id: String,
    pub object_type: String,
    pub user_id: String,
    pub title: Option<String>,
    pub canonical_url: Option<String>,
    pub source_platform: Option<String>,
    pub author: Option<String>,
    pub privacy_level: String,
    pub lifecycle_status: String,
    pub captured_at: String,
    pub updated_at: String,
    pub snapshot: CaptureSnapshotSubmission,
    pub parsed_document: Option<CaptureParsedDocumentSubmission>,
    pub job: CaptureBackgroundJobSubmission,
    pub events: Vec<CaptureDomainEventSubmission>,
}

#[derive(Debug, Clone)]
pub struct CaptureSnapshotSubmission {
    pub id: String,
    pub snapshot_type: String,
    pub storage_uri: String,
    pub content_hash: String,
    pub parser_id: Option<String>,
    pub parser_version: Option<String>,
    pub captured_at: String,
}

#[derive(Debug, Clone)]
pub struct CaptureParsedDocumentSubmission {
    pub id: String,
    pub title: Option<String>,
    pub text_content: String,
    pub markdown_content: Option<String>,
    pub language: Option<String>,
    pub word_count: i64,
    pub content_hash: String,
    pub parser_id: String,
    pub parser_version: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CaptureBackgroundJobSubmission {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub payload_json: String,
    pub max_attempts: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CaptureDomainEventSubmission {
    pub id: String,
    pub event_type: String,
    pub event_version: i64,
    pub user_id: String,
    pub payload_json: String,
    pub occurred_at: String,
}
