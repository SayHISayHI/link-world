use crate::domain::knowledge::EvidenceItem;
use serde::Serialize;
use serde_json::Value;

pub const EVALUATION_CAPABILITY_SCHEMA_VERSION: i64 = 1;
pub const EVALUATION_PLAN_SCHEMA_VERSION: i64 = 1;
pub const EVALUATION_INPUT_SCHEMA_VERSION: i64 = 1;
pub const EVALUATION_OUTPUT_SCHEMA_VERSION: i64 = 1;
pub const EVALUATION_TRACE_SCHEMA_VERSION: i64 = 1;
pub const DEFAULT_EVALUATION_TIMEOUT_MS: i64 = 2_000;
pub const GITHUB_EVALUATION_TIMEOUT_MS: i64 = 15_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerEvaluationResponse {
    pub run_id: String,
    pub job_id: String,
    pub request_id: String,
    pub correlation_id: String,
    pub retry_of_run_id: Option<String>,
    pub status: String,
    pub reused: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatorCapability {
    pub schema_version: i64,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub display_name: String,
    pub supported_object_types: Vec<String>,
    pub execution_kind: String,
    pub requires_network: bool,
    pub requires_model: bool,
    pub requires_sandbox: bool,
    pub plan_schema_version: i64,
    pub input_schema_version: i64,
    pub output_schema_version: i64,
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
    pub schema_version: i64,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub steps: Vec<String>,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationOutput {
    pub schema_version: i64,
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
    pub request_id: String,
    pub correlation_id: String,
    pub object_id: String,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub plan_schema_version: i64,
    pub input_schema_version: i64,
    pub output_schema_version: i64,
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
pub struct EvaluationRunReservation {
    pub id: String,
    pub request_id: String,
    pub correlation_id: String,
    pub job_id: String,
    pub object_id: String,
    pub retry_of_run_id: Option<String>,
    pub requested_evaluator_type: String,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub plan_schema_version: i64,
    pub input_schema_version: i64,
    pub output_schema_version: i64,
    pub trace_id: String,
    pub execution_kind: String,
    pub input_hash: String,
    pub timeout_ms: i64,
    pub plan_json: String,
    pub input_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct EvaluationOperation {
    pub run_id: String,
    pub request_id: String,
    pub correlation_id: String,
    pub job_id: String,
    pub object_id: String,
    pub retry_of_run_id: Option<String>,
    pub requested_evaluator_type: String,
    pub evaluator_type: String,
    pub evaluator_version: String,
    pub status: String,
}
#[derive(Debug, Clone)]
pub struct EvaluationRetryCandidate {
    pub run_id: String,
    pub object_id: String,
    pub requested_evaluator_type: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct EvaluationFailureSubmission {
    pub user_id: String,
    pub run_id: String,
    pub job_id: String,
    pub correlation_id: String,
    pub object_id: String,
    pub evaluator_type: String,
    pub error_code: String,
    pub latency_ms: i64,
    pub completed_at: String,
}

#[derive(Debug, Clone)]
pub struct EvaluationTraceCompletion {
    pub output_hash: String,
    pub latency_ms: i64,
    pub completed_at: String,
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
