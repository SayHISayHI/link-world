use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteObjectMode {
    SoftDelete,
    Purge,
    ExportThenDelete,
}

impl DeleteObjectMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SoftDelete => "soft_delete",
            Self::Purge => "purge",
            Self::ExportThenDelete => "export_then_delete",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteObjectResponse {
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeObject {
    pub id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub object_type: String,
    pub title: Option<String>,
    pub canonical_url: Option<String>,
    pub source_platform: Option<String>,
    pub author: Option<String>,
    pub privacy_level: String,
    pub lifecycle_status: String,
    pub failure_reason: Option<String>,
    pub captured_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewKnowledgeObject {
    pub user_id: String,
    pub object_type: String,
    pub title: Option<String>,
    pub canonical_url: Option<String>,
    pub source_platform: Option<String>,
    pub author: Option<String>,
    pub privacy_level: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshot {
    pub id: String,
    pub object_id: String,
    pub snapshot_type: String,
    pub storage_uri: String,
    pub content_hash: String,
    pub parser_id: Option<String>,
    pub parser_version: Option<String>,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDocument {
    pub id: String,
    pub object_id: String,
    pub source_snapshot_id: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "text")]
    pub text_content: String,
    #[serde(rename = "markdown")]
    pub markdown_content: Option<String>,
    pub language: Option<String>,
    pub word_count: Option<i64>,
    pub content_hash: String,
    pub parser_id: String,
    pub parser_version: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AITrace {
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIAnalysis {
    pub id: String,
    pub object_id: String,
    pub parsed_document_id: Option<String>,
    pub analysis_type: String,
    pub schema_version: i64,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub key_points: Vec<Value>,
    pub claims: Vec<Value>,
    pub action_items: Vec<Value>,
    pub risks: Vec<Value>,
    pub quality_score: Option<f64>,
    pub confidence: Option<f64>,
    pub trace: Option<AITrace>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub source: String,
    pub text: String,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationArtifact {
    pub kind: String,
    pub uri: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationRun {
    pub id: String,
    pub object_id: String,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub status: String,
    pub score: Option<f64>,
    pub verdict: String,
    pub dimensions: Value,
    pub evidence: Vec<EvidenceItem>,
    pub artifacts: Vec<EvaluationArtifact>,
    pub limitations: Vec<String>,
    pub next_actions: Vec<Value>,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeObjectDetail {
    pub object: KnowledgeObject,
    pub parsed_document: Option<ParsedDocument>,
    pub snapshots: Vec<SourceSnapshot>,
    pub ai_analyses: Vec<AIAnalysis>,
    pub evaluations: Vec<EvaluationRun>,
}
