use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderConfig {
    pub provider: String,
    pub chat_base_url: Option<String>,
    pub embeddings_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub default_chat_model: Option<String>,
    pub default_embedding_model: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StoredModelProviderConfig {
    pub id: String,
    pub provider: String,
    pub chat_base_url: Option<String>,
    pub embeddings_base_url: Option<String>,
    pub default_chat_model: Option<String>,
    pub default_embedding_model: Option<String>,
    pub capabilities: Vec<String>,
    pub secret_ref: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIEnrichmentRunResult {
    pub job_id: String,
    pub analysis_id: Option<String>,
    pub status: String,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AIEnrichmentInput {
    pub object_id: String,
    pub object_type: String,
    pub title: Option<String>,
    pub canonical_url: Option<String>,
    pub privacy_level: String,
    pub parsed_document_id: String,
    pub source_snapshot_id: Option<String>,
    pub text_content: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIModelAnalysisOutput {
    pub summary: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub key_points: Vec<Value>,
    #[serde(default)]
    pub claims: Vec<Value>,
    #[serde(default)]
    pub action_items: Vec<Value>,
    #[serde(default)]
    pub risks: Vec<Value>,
    #[serde(default)]
    pub quality_score: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct AIAnalysisSubmission {
    pub id: String,
    pub object_id: String,
    pub parsed_document_id: String,
    pub analysis_type: String,
    pub schema_version: i64,
    pub summary: String,
    pub category: Option<String>,
    pub tags_json: String,
    pub key_points_json: String,
    pub claims_json: String,
    pub action_items_json: String,
    pub risks_json: String,
    pub quality_score: Option<f64>,
    pub confidence: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct AITraceSubmission {
    pub id: String,
    pub analysis_id: String,
    pub object_id: String,
    pub provider: String,
    pub model: String,
    pub capability: String,
    pub prompt_template_id: Option<String>,
    pub prompt_template_version: Option<String>,
    pub input_snapshot_id: Option<String>,
    pub input_parsed_document_id: Option<String>,
    pub input_hash: Option<String>,
    pub output_hash: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub latency_ms: Option<i64>,
    pub created_at: String,
}
