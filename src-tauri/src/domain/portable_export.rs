use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortableExportFormat {
    Json,
    Markdown,
    Both,
}

impl PortableExportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json_directory",
            Self::Markdown => "markdown_directory",
            Self::Both => "markdown_json_directory",
        }
    }

    pub fn includes_json(self) -> bool {
        matches!(self, Self::Json | Self::Both)
    }

    pub fn includes_markdown(self) -> bool {
        matches!(self, Self::Markdown | Self::Both)
    }
}

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
