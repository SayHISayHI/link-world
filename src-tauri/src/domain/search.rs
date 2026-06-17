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
