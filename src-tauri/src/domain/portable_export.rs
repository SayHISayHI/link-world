use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableExportSummary {
    pub export_id: String,
    pub export_root: String,
    pub format: String,
    pub object_count: usize,
    pub skipped_secret_count: usize,
    pub markdown_file_count: usize,
    pub json_file_count: usize,
    pub created_at: String,
}
