use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundJob {
    pub id: String,
    #[serde(rename = "type")]
    pub job_type: String,
    pub status: String,
    pub object_id: Option<String>,
    pub attempt_count: i64,
    pub max_attempts: i64,
    pub next_run_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RetriedBackgroundJob {
    pub id: String,
    pub job_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredEvaluationOperation {
    pub run_id: String,
    pub job_id: String,
    pub correlation_id: String,
    pub object_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupJobRecoverySummary {
    pub requeued_count: u64,
    pub failed_count: u64,
    pub object_failed_count: u64,
    pub evaluation_failed_count: u64,
    pub recovered_evaluations: Vec<RecoveredEvaluationOperation>,
}
