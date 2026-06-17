use crate::domain::knowledge::EvidenceItem;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerEvaluationResponse {
    pub run_id: String,
}

#[derive(Debug, Clone)]
pub struct EvaluationInput {
    pub object_id: String,
    pub user_id: String,
    pub object_type: String,
    pub title: Option<String>,
    pub canonical_url: Option<String>,
    pub privacy_level: String,
    pub parsed_document_id: String,
    pub text_content: String,
    pub word_count: Option<i64>,
    pub content_hash: String,
    pub latest_ai_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationPlan {
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub steps: Vec<String>,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationOutput {
    pub score: f64,
    pub verdict: String,
    pub dimensions: Value,
    pub evidence: Vec<EvidenceItem>,
    pub limitations: Vec<String>,
    pub next_actions: Vec<Value>,
    pub report: Value,
}

#[derive(Debug, Clone)]
pub struct EvaluationRunSubmission {
    pub id: String,
    pub object_id: String,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub status: String,
    pub plan_json: String,
    pub input_json: String,
    pub output_json: String,
    pub dimensions_json: String,
    pub evidence_json: String,
    pub limitations_json: String,
    pub next_actions_json: String,
    pub score: Option<f64>,
    pub verdict: String,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EvaluationArtifactSubmission {
    pub id: String,
    pub evaluation_run_id: String,
    pub artifact_type: String,
    pub storage_uri: String,
    pub content_hash: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}
