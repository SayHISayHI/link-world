use crate::domain::knowledge::KnowledgeObject;
use serde::Serialize;

pub const SEARCH_REBUILD_FAILURE_REASON: &str =
    "search.rebuild_failed: Search index rebuild failed. Retry from Settings and inspect Diagnostics.";
pub const SEARCH_REINDEX_FAILURE_REASON: &str =
    "search.reindex_failed: Object reindex failed. Retry from the object or inspect Diagnostics.";
pub const SEARCH_QUERY_FAILURE_REASON: &str =
    "search.query_failed: Search could not be completed. Refine the query or retry.";
pub const SEARCH_HEALTH_FAILURE_REASON: &str =
    "search.health_failed: Search index health could not be checked. Retry or inspect Diagnostics.";

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
