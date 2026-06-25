use crate::domain::knowledge::KnowledgeObject;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub object: KnowledgeObject,
    pub matched_fields: Vec<String>,
    pub snippet: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildSearchIndexResponse {
    pub job_id: String,
    pub status: String,
    pub stage: String,
    pub expected_objects: i64,
    pub indexed_objects: i64,
    pub progress_percent: f64,
    pub cancellable: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReindexObjectResponse {
    pub job_id: String,
    pub object_id: String,
    pub indexed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexHealthResponse {
    pub healthy: bool,
    pub expected_indexed_objects: i64,
    pub actual_indexed_rows: i64,
    pub missing_objects: i64,
    pub stale_objects: i64,
    pub orphaned_rows: i64,
    pub duplicate_rows: i64,
    pub missing_object_ids: Vec<String>,
    pub stale_object_ids: Vec<String>,
    pub orphaned_object_ids: Vec<String>,
    pub duplicate_object_ids: Vec<String>,
}
