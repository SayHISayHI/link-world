use crate::domain::capture::{
    CaptureBackgroundJobSubmission, CaptureDomainEventSubmission, CaptureParsedDocumentSubmission,
    CaptureSnapshotSubmission, CaptureSubmission, RawCaptureItem, SubmitCaptureResponse,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::capture::CaptureRepository;
use crate::state::AppState;
use crate::storage::object_store::{sha256_hex, ObjectStore};
use chrono::Utc;
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

const LOCAL_USER_ID: &str = "local";
const INLINE_CAPTURE_PARSER_ID: &str = "builtin.inline_capture_parser";
const INLINE_CAPTURE_PARSER_VERSION: &str = "0.1.0";
const MAX_RAW_CAPTURE_BYTES: usize = 5 * 1024 * 1024;

pub struct CaptureService {
    pool: SqlitePool,
    object_store: ObjectStore,
}

impl CaptureService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            pool: state.database()?.pool().clone(),
            object_store: state.object_store()?.clone(),
        })
    }

    #[cfg(test)]
    pub fn new(pool: SqlitePool, object_store: ObjectStore) -> Self {
        Self { pool, object_store }
    }

    pub async fn submit(&self, item: RawCaptureItem) -> AppResult<SubmitCaptureResponse> {
        validate_capture_item(&item)?;

        let object_id = Uuid::new_v4().to_string();
        let snapshot_id = Uuid::new_v4().to_string();
        let job_id = Uuid::new_v4().to_string();
        let now = item.captured_at.clone().unwrap_or_else(|| Utc::now().to_rfc3339());
        let snapshot_bytes = serde_json::to_vec_pretty(&item)
            .map_err(|error| AppError::Unknown(error.to_string()))?;

        if snapshot_bytes.len() > MAX_RAW_CAPTURE_BYTES {
            return Err(AppError::PolicyDenied(format!(
                "capture snapshot exceeds {} bytes",
                MAX_RAW_CAPTURE_BYTES
            )));
        }

        let stored_snapshot = self
            .object_store
            .write_capture_snapshot(&object_id, &snapshot_id, snapshot_bytes)
            .await?;

        let parsed = build_inline_parsed_document(&item, &now)?;
        let lifecycle_status = if parsed.is_some() { "parsed" } else { "captured" }.to_string();
        let job_type = if parsed.is_some() {
            "search.reindex_object"
        } else {
            "capture.fetch_url"
        };

        let snapshot = CaptureSnapshotSubmission {
            id: snapshot_id.clone(),
            snapshot_type: infer_snapshot_type(&item).to_string(),
            storage_uri: stored_snapshot.storage_uri,
            content_hash: stored_snapshot.content_hash,
            parser_id: parsed.as_ref().map(|_| INLINE_CAPTURE_PARSER_ID.to_string()),
            parser_version: parsed
                .as_ref()
                .map(|_| INLINE_CAPTURE_PARSER_VERSION.to_string()),
            captured_at: now.clone(),
        };

        let submission = CaptureSubmission {
            object_id: object_id.clone(),
            object_type: infer_object_type(&item),
            user_id: item.user_id.clone().unwrap_or_else(|| LOCAL_USER_ID.to_string()),
            title: normalized_title(&item),
            canonical_url: item.canonical_url.clone().or_else(|| item.source_url.clone()),
            source_platform: item.source_platform.clone(),
            author: item.author.clone(),
            privacy_level: item.privacy_level.clone(),
            lifecycle_status,
            captured_at: now.clone(),
            updated_at: now.clone(),
            snapshot,
            parsed_document: parsed,
            job: CaptureBackgroundJobSubmission {
                id: job_id.clone(),
                job_type: job_type.to_string(),
                status: "queued".to_string(),
                payload_json: build_job_payload(&object_id, &snapshot_id),
                max_attempts: 3,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            events: build_domain_events(&item, &object_id, &snapshot_id, &now),
        };

        let response = SubmitCaptureResponse {
            object_id: object_id.clone(),
            snapshot_id,
            parsed_document_id: submission
                .parsed_document
                .as_ref()
                .map(|document| document.id.clone()),
            job_id,
        };

        let mut tx = self.pool.begin().await?;
        CaptureRepository::insert_submission(&mut tx, &submission).await?;
        tx.commit().await?;

        Ok(response)
    }
}

fn validate_capture_item(item: &RawCaptureItem) -> AppResult<()> {
    if !item.permission_context.user_confirmed {
        return Err(AppError::PolicyDenied(
            "capture requires explicit user confirmation".to_string(),
        ));
    }

    if !is_allowed_acquisition_mode(&item.permission_context.acquisition_mode) {
        return Err(AppError::PolicyDenied(format!(
            "unsupported acquisition mode: {}",
            item.permission_context.acquisition_mode
        )));
    }

    if !is_allowed_privacy_level(&item.privacy_level) {
        return Err(AppError::PolicyDenied(format!(
            "unsupported privacy level: {}",
            item.privacy_level
        )));
    }

    if !has_meaningful_capture_input(item) {
        return Err(AppError::PolicyDenied(
            "capture item must include sourceUrl, canonicalUrl, rawText, or rawHtml".to_string(),
        ));
    }

    Ok(())
}

fn is_allowed_acquisition_mode(value: &str) -> bool {
    matches!(
        value,
        "user_action" | "official_api" | "import" | "local_automation"
    )
}

fn is_allowed_privacy_level(value: &str) -> bool {
    matches!(value, "public" | "personal" | "sensitive" | "secret")
}

fn has_meaningful_capture_input(item: &RawCaptureItem) -> bool {
    has_text(&item.source_url)
        || has_text(&item.canonical_url)
        || has_text(&item.raw_text)
        || has_text(&item.raw_html)
}

fn has_text(value: &Option<String>) -> bool {
    value.as_ref().is_some_and(|text| !text.trim().is_empty())
}

fn build_inline_parsed_document(
    item: &RawCaptureItem,
    created_at: &str,
) -> AppResult<Option<CaptureParsedDocumentSubmission>> {
    let Some(text_content) = parsed_text_content(item) else {
        return Ok(None);
    };

    let text_content = text_content.trim().to_string();
    if text_content.is_empty() {
        return Ok(None);
    }

    let content_hash = sha256_hex(text_content.as_bytes());
    let word_count = text_content.split_whitespace().count() as i64;

    Ok(Some(CaptureParsedDocumentSubmission {
        id: Uuid::new_v4().to_string(),
        title: normalized_title(item),
        text_content,
        markdown_content: item.raw_text.clone(),
        language: None,
        word_count,
        content_hash,
        parser_id: INLINE_CAPTURE_PARSER_ID.to_string(),
        parser_version: INLINE_CAPTURE_PARSER_VERSION.to_string(),
        created_at: created_at.to_string(),
    }))
}

fn parsed_text_content(item: &RawCaptureItem) -> Option<String> {
    if has_text(&item.raw_text) {
        return item.raw_text.clone();
    }

    item.raw_html
        .as_ref()
        .map(|html| strip_html_tags(html).trim().to_string())
}

fn strip_html_tags(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut in_tag = false;

    for character in html.chars() {
        match character {
            '<' => {
                in_tag = true;
                output.push(' ');
            }
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }

    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalized_title(item: &RawCaptureItem) -> Option<String> {
    item.title
        .as_ref()
        .map(|title| title.trim())
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| extract_title_from_html(item))
}

fn extract_title_from_html(item: &RawCaptureItem) -> Option<String> {
    let html = item.raw_html.as_ref()?;
    let lower_html = html.to_lowercase();
    let start = lower_html.find("<title>")? + "<title>".len();
    let end = lower_html[start..].find("</title>")? + start;
    let title = strip_html_tags(&html[start..end]);

    if title.trim().is_empty() {
        None
    } else {
        Some(title.trim().to_string())
    }
}

fn infer_snapshot_type(item: &RawCaptureItem) -> &'static str {
    if has_text(&item.raw_html) {
        "html"
    } else if has_text(&item.raw_text) {
        "text"
    } else {
        "json"
    }
}

fn infer_object_type(item: &RawCaptureItem) -> String {
    if let Some(object_type) = item
        .metadata
        .get("objectType")
        .and_then(|value| value.as_str())
        .filter(|value| is_allowed_object_type(value))
    {
        return object_type.to_string();
    }

    let url = item
        .canonical_url
        .as_deref()
        .or(item.source_url.as_deref())
        .unwrap_or_default()
        .to_lowercase();

    if url.contains("github.com/") {
        "github_repo".to_string()
    } else if matches!(item.source_type.as_str(), "selection" | "dom") {
        "article".to_string()
    } else if item.source_type == "file" {
        "file".to_string()
    } else {
        "article".to_string()
    }
}

fn is_allowed_object_type(value: &str) -> bool {
    matches!(
        value,
        "article"
            | "social_post"
            | "thread"
            | "prompt"
            | "github_repo"
            | "tool"
            | "tutorial"
            | "paper"
            | "video"
            | "podcast"
            | "conversation"
            | "note"
            | "dataset"
            | "file"
            | "collection"
    )
}

fn build_job_payload(object_id: &str, snapshot_id: &str) -> String {
    json!({
        "objectId": object_id,
        "snapshotId": snapshot_id,
    })
    .to_string()
}

fn build_domain_events(
    item: &RawCaptureItem,
    object_id: &str,
    snapshot_id: &str,
    occurred_at: &str,
) -> Vec<CaptureDomainEventSubmission> {
    let user_id = item.user_id.clone().unwrap_or_else(|| LOCAL_USER_ID.to_string());
    let mut events = Vec::with_capacity(3);

    events.push(CaptureDomainEventSubmission {
        id: Uuid::new_v4().to_string(),
        event_type: "capture.submitted".to_string(),
        event_version: 1,
        user_id: user_id.clone(),
        payload_json: json!({
            "sourceType": item.source_type,
            "sourcePlatform": item.source_platform,
            "sourceUrl": item.source_url,
            "canonicalUrl": item.canonical_url,
            "snapshotId": snapshot_id,
        })
        .to_string(),
        occurred_at: occurred_at.to_string(),
    });

    events.push(CaptureDomainEventSubmission {
        id: Uuid::new_v4().to_string(),
        event_type: "snapshot.created".to_string(),
        event_version: 1,
        user_id: user_id.clone(),
        payload_json: json!({
            "snapshotId": snapshot_id,
        })
        .to_string(),
        occurred_at: occurred_at.to_string(),
    });

    if has_text(&item.raw_text) || has_text(&item.raw_html) {
        events.push(CaptureDomainEventSubmission {
            id: Uuid::new_v4().to_string(),
            event_type: "object.parsed".to_string(),
            event_version: 1,
            user_id,
            payload_json: json!({
                "objectId": object_id,
                "parserId": INLINE_CAPTURE_PARSER_ID,
            })
            .to_string(),
            occurred_at: occurred_at.to_string(),
        });
    }

    events
}

#[cfg(test)]
mod tests {
    use super::CaptureService;
    use crate::domain::capture::{PermissionContext, RawCaptureItem};
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use serde_json::json;

    #[tokio::test]
    async fn submit_text_capture_writes_object_snapshot_document_event_and_job() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);

        let response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "selection".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some("https://example.com/post".to_string()),
                canonical_url: None,
                title: Some("Useful technique".to_string()),
                author: Some("Author".to_string()),
                captured_at: Some("2026-06-16T00:00:00Z".to_string()),
                raw_html: None,
                raw_text: Some("Save useful content before it disappears.".to_string()),
                assets: Vec::new(),
                metadata: json!({ "objectType": "article" }),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("capture should succeed");

        assert!(uuid::Uuid::parse_str(&response.object_id).is_ok());
        assert!(response.parsed_document_id.is_some());

        let object_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_objects")
            .fetch_one(database.pool())
            .await
            .expect("object count should be readable");
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM source_snapshots")
            .fetch_one(database.pool())
            .await
            .expect("snapshot count should be readable");
        let parsed_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM parsed_documents")
            .fetch_one(database.pool())
            .await
            .expect("parsed document count should be readable");
        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM domain_events")
            .fetch_one(database.pool())
            .await
            .expect("event count should be readable");
        let job_type: String =
            sqlx::query_scalar("SELECT job_type FROM background_jobs WHERE object_id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("job type should be readable");

        assert_eq!(object_count, 1);
        assert_eq!(snapshot_count, 1);
        assert_eq!(parsed_count, 1);
        assert_eq!(event_count, 3);
        assert_eq!(job_type, "search.reindex_object");
    }

    #[tokio::test]
    async fn submit_capture_rejects_unconfirmed_user_action() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);

        let result = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: None,
                source_url: Some("https://example.com".to_string()),
                canonical_url: None,
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: PermissionContext {
                    user_confirmed: false,
                    ..confirmed_permission()
                },
            })
            .await;

        assert!(result.is_err());
    }

    fn confirmed_permission() -> PermissionContext {
        PermissionContext {
            acquisition_mode: "user_action".to_string(),
            user_confirmed: true,
            platform_terms_hint: None,
            allowed_for_cloud_processing: false,
            allowed_for_third_party_ai: false,
        }
    }

    fn test_object_store() -> ObjectStore {
        let root = std::env::temp_dir().join(format!("link-world-test-{}", uuid::Uuid::new_v4()));
        ObjectStore::initialize(root).expect("object store should initialize")
    }
}
