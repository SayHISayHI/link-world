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
    pub indexed_objects: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReindexObjectResponse {
    pub job_id: String,
    pub object_id: String,
    pub indexed: bool,
}
