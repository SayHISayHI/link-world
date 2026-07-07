use serde::{Deserialize, Serialize};

pub const LOCAL_USER_ID: &str = "local-user";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibraryViewKind {
    System,
    Collection,
    Tag,
    Smart,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryViewRef {
    pub kind: LibraryViewKind,
    pub id: String,
}

impl Default for LibraryViewRef {
    fn default() -> Self {
        Self {
            kind: LibraryViewKind::System,
            id: "all".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFilters {
    #[serde(default)]
    pub object_types: Vec<String>,
    #[serde(default)]
    pub lifecycle_statuses: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub privacy_levels: Vec<String>,
    pub quality_min: Option<f64>,
    pub quality_max: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQuery {
    #[serde(default)]
    pub view: LibraryViewRef,
    #[serde(default)]
    pub filters: LibraryFilters,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationItem {
    pub id: String,
    pub label: String,
    pub count: i64,
    pub kind: LibraryViewKind,
    pub icon_key: Option<String>,
    pub color_token: Option<String>,
    pub collection_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryNavigation {
    pub system_views: Vec<NavigationItem>,
    pub collections: Vec<NavigationItem>,
    pub topics: Vec<NavigationItem>,
    pub smart_views: Vec<NavigationItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub normalized_name: String,
    pub source: String,
    pub color_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub collection_type: String,
    pub icon_key: Option<String>,
    pub color_token: Option<String>,
    pub query_json: Option<String>,
    pub sort_order: i64,
    pub is_pinned: bool,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSuggestion {
    pub id: String,
    pub object_id: String,
    pub analysis_id: String,
    pub name: String,
    pub normalized_name: String,
    pub confidence: Option<f64>,
    pub rationale: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOrganization {
    pub object_id: String,
    pub triage_status: String,
    pub tags: Vec<Tag>,
    pub collections: Vec<Collection>,
    pub tag_suggestions: Vec<TagSuggestion>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub color_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionInput {
    pub collection_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub color_token: Option<String>,
    pub is_pinned: Option<bool>,
    pub sort_order: Option<i64>,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartViewRule {
    pub schema_version: i64,
    #[serde(default)]
    pub object_types: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub minimum_quality: Option<f64>,
    pub analysis_state: Option<String>,
    pub evaluation_state: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSmartViewInput {
    pub name: String,
    pub description: Option<String>,
    pub rule: SmartViewRule,
}

#[derive(Debug, Clone)]
pub struct NewTagSuggestion {
    pub id: String,
    pub object_id: String,
    pub analysis_id: String,
    pub name: String,
    pub normalized_name: String,
    pub confidence: Option<f64>,
    pub rationale: Option<String>,
    pub created_at: String,
}
