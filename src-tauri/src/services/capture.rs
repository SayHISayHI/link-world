use crate::domain::capture::{
    CaptureBackgroundJobSubmission, CaptureDomainEventSubmission, CaptureFetchJobRecord,
    CaptureFetchJobRunResult, CaptureParsedDocumentSubmission, CaptureSnapshotSubmission,
    CaptureSubmission, RawCaptureItem, SubmitCaptureResponse,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::capture::{
    CaptureFetchCompletion, CaptureRepository, ExistingCaptureRecord,
};
use crate::services::ai::{spawn_ai_enrichment_runner, AIEnrichmentService};
use crate::services::document_parser::{parse_html_document, DocumentHints, ParsedWebDocument};
use crate::state::AppState;
use crate::storage::object_store::{sha256_hex, ObjectStore};
use chrono::Utc;
use reqwest::Url;
use serde_json::json;
use sqlx::SqlitePool;
use std::time::Duration;
use tauri::Emitter;
use uuid::Uuid;

const LOCAL_USER_ID: &str = "local";
const INLINE_CAPTURE_PARSER_ID: &str = "builtin.inline_text_parser";
const INLINE_CAPTURE_PARSER_VERSION: &str = "0.1.0";
const HTML_FETCH_PARSER_ID: &str = "builtin.document_html_parser";
const HTML_FETCH_PARSER_VERSION: &str = "0.3.0";
const MAX_RAW_CAPTURE_BYTES: usize = 5 * 1024 * 1024;
const MAX_FETCH_BYTES: usize = 5 * 1024 * 1024;
const FETCH_TIMEOUT_SECONDS: u64 = 20;

#[derive(Clone)]
pub struct CaptureService {
    pool: SqlitePool,
    object_store: ObjectStore,
    http_client: reqwest::Client,
}

impl CaptureService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            pool: state.database()?.pool().clone(),
            object_store: state.object_store()?.clone(),
            http_client: build_http_client()?,
        })
    }

    pub fn new(pool: SqlitePool, object_store: ObjectStore) -> Self {
        Self {
            pool,
            object_store,
            http_client: build_http_client().expect("test HTTP client should build"),
        }
    }

    pub async fn submit(&self, item: RawCaptureItem) -> AppResult<SubmitCaptureResponse> {
        validate_capture_item(&item)?;

        let user_id = item
            .user_id
            .clone()
            .unwrap_or_else(|| LOCAL_USER_ID.to_string());
        let canonical_url = normalized_capture_canonical_url(&item);

        if let Some(existing) = self
            .find_duplicate_url_capture(&item, &user_id, canonical_url.as_deref())
            .await?
        {
            return Ok(existing);
        }

        let object_id = Uuid::new_v4().to_string();
        let snapshot_id = Uuid::new_v4().to_string();
        let job_id = Uuid::new_v4().to_string();
        let correlation_id = Uuid::new_v4().to_string();
        let now = item
            .captured_at
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339());
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
        let lifecycle_status = if parsed.is_some() {
            "parsed"
        } else {
            "captured"
        }
        .to_string();
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
            parser_id: parsed.as_ref().map(|document| document.parser_id.clone()),
            parser_version: parsed
                .as_ref()
                .map(|document| document.parser_version.clone()),
            captured_at: now.clone(),
        };

        let submission = CaptureSubmission {
            object_id: object_id.clone(),
            object_type: infer_object_type(&item),
            user_id,
            title: normalized_title(&item)
                .or_else(|| parsed.as_ref().and_then(|document| document.title.clone())),
            canonical_url,
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
                payload_json: build_job_payload(&object_id, &snapshot_id, &correlation_id),
                max_attempts: 3,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            events: build_domain_events(&item, &object_id, &snapshot_id, &correlation_id, &now),
        };

        let response = SubmitCaptureResponse {
            object_id: object_id.clone(),
            snapshot_id: Some(snapshot_id),
            parsed_document_id: submission
                .parsed_document
                .as_ref()
                .map(|document| document.id.clone()),
            job_id: Some(job_id),
            deduplicated: false,
        };

        let mut tx = self.pool.begin().await?;
        CaptureRepository::insert_submission(&mut tx, &submission).await?;
        tx.commit().await?;

        Ok(response)
    }

    async fn find_duplicate_url_capture(
        &self,
        item: &RawCaptureItem,
        user_id: &str,
        canonical_url: Option<&str>,
    ) -> AppResult<Option<SubmitCaptureResponse>> {
        if !is_deduplicated_url_capture(item) {
            return Ok(None);
        }

        let Some(canonical_url) = canonical_url else {
            return Ok(None);
        };

        let mut tx = self.pool.begin().await?;
        let existing =
            CaptureRepository::find_active_by_canonical_url(&mut tx, user_id, canonical_url)
                .await?;
        tx.commit().await?;

        Ok(existing.map(existing_capture_response))
    }

    pub async fn run_fetch_job(&self, job_id: &str) -> AppResult<Option<CaptureFetchJobRunResult>> {
        let now = Utc::now().to_rfc3339();
        let locked_by = format!("link-world-{}", Uuid::new_v4());
        let mut tx = self.pool.begin().await?;
        let job =
            CaptureRepository::claim_fetch_job_by_id(&mut tx, job_id, &locked_by, &now).await?;
        tx.commit().await?;

        let Some(job) = job else {
            return Ok(None);
        };

        let outcome = self.fetch_and_parse_job(&job).await;

        match outcome {
            Ok(parsed) => self.complete_fetch_job(job, parsed).await.map(Some),
            Err(error) => self
                .fail_fetch_job(job, capture_failure_reason(&error))
                .await
                .map(Some),
        }
    }

    async fn fetch_and_parse_job(
        &self,
        job: &CaptureFetchJobRecord,
    ) -> AppResult<FetchedHtmlDocument> {
        if job.attempt_count >= job.max_attempts {
            return Err(AppError::PolicyDenied(
                "capture fetch job exceeded max attempts".to_string(),
            ));
        }

        let url = job
            .canonical_url
            .as_deref()
            .ok_or_else(|| AppError::ParseFailed("capture has no canonical URL".to_string()))?;
        let url = Url::parse(url).map_err(|error| AppError::ParseFailed(error.to_string()))?;

        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::PolicyDenied(format!(
                "unsupported URL scheme: {}",
                url.scheme()
            )));
        }

        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = response.status();

        if !status.is_success() {
            return Err(AppError::ParseFailed(format!("URL returned HTTP {status}")));
        }

        let bytes = response.bytes().await.map_err(map_reqwest_error)?;
        if bytes.len() > MAX_FETCH_BYTES {
            return Err(AppError::PolicyDenied(format!(
                "fetched HTML exceeds {} bytes",
                MAX_FETCH_BYTES
            )));
        }

        let html = String::from_utf8_lossy(&bytes).to_string();
        parse_fetched_html(html).await
    }

    async fn complete_fetch_job(
        &self,
        job: CaptureFetchJobRecord,
        parsed: FetchedHtmlDocument,
    ) -> AppResult<CaptureFetchJobRunResult> {
        let now = Utc::now().to_rfc3339();
        let snapshot_id = Uuid::new_v4().to_string();
        let parsed_document_id = Uuid::new_v4().to_string();
        let stored_snapshot = self
            .object_store
            .write_capture_artifact(
                &job.object_id,
                &snapshot_id,
                "html",
                parsed.raw_html.as_bytes().to_vec(),
            )
            .await?;

        let text_content = parsed.text_content.trim().to_string();
        if text_content.is_empty() {
            return self
                .fail_fetch_job(job, capture_no_readable_text_failure_reason())
                .await;
        }

        let snapshot = CaptureSnapshotSubmission {
            id: snapshot_id.clone(),
            snapshot_type: "html".to_string(),
            storage_uri: stored_snapshot.storage_uri,
            content_hash: stored_snapshot.content_hash,
            parser_id: Some(HTML_FETCH_PARSER_ID.to_string()),
            parser_version: Some(HTML_FETCH_PARSER_VERSION.to_string()),
            captured_at: now.clone(),
        };
        let parsed_document = CaptureParsedDocumentSubmission {
            id: parsed_document_id.clone(),
            title: parsed.title.clone(),
            text_content,
            markdown_content: Some(parsed.markdown_content),
            language: parsed.language,
            word_count: parsed.word_count,
            content_hash: parsed.content_hash,
            parser_id: HTML_FETCH_PARSER_ID.to_string(),
            parser_version: HTML_FETCH_PARSER_VERSION.to_string(),
            created_at: now.clone(),
        };
        let events = build_fetch_success_events(&job, &snapshot_id, &parsed_document_id, &now);

        let mut tx = self.pool.begin().await?;
        CaptureRepository::complete_fetch_job(
            &mut tx,
            CaptureFetchCompletion {
                job_id: &job.id,
                object_id: &job.object_id,
                user_id: &job.user_id,
                title: parsed.title.as_deref(),
                author: parsed.author.as_deref(),
                snapshot: &snapshot,
                parsed_document: &parsed_document,
                events: &events,
                now: &now,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(CaptureFetchJobRunResult {
            job_id: job.id,
            object_id: job.object_id,
            status: "succeeded".to_string(),
            lifecycle_status: "parsed".to_string(),
            parsed_document_id: Some(parsed_document_id),
            failure_reason: None,
        })
    }

    async fn fail_fetch_job(
        &self,
        job: CaptureFetchJobRecord,
        failure_reason: String,
    ) -> AppResult<CaptureFetchJobRunResult> {
        let now = Utc::now().to_rfc3339();
        let event = build_fetch_failed_event(&job, &failure_reason, &now);
        let mut tx = self.pool.begin().await?;

        CaptureRepository::fail_fetch_job(
            &mut tx,
            &job.id,
            &job.object_id,
            &job.user_id,
            &failure_reason,
            &event,
            &now,
        )
        .await?;
        tx.commit().await?;

        Ok(CaptureFetchJobRunResult {
            job_id: job.id,
            object_id: job.object_id,
            status: "failed".to_string(),
            lifecycle_status: "failed".to_string(),
            parsed_document_id: None,
            failure_reason: Some(failure_reason),
        })
    }
}

pub fn spawn_fetch_job_runner(
    app_handle: tauri::AppHandle,
    service: CaptureService,
    ai_service: AIEnrichmentService,
    job_id: String,
) {
    tauri::async_runtime::spawn(async move {
        let result = service.run_fetch_job(&job_id).await;
        let ai_object_id = result
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .filter(|run| run.status == "succeeded" && run.parsed_document_id.is_some())
            .map(|run| run.object_id.clone());

        let payload = match result {
            Ok(Some(result)) => json!({
                "jobId": result.job_id,
                "status": result.status,
                "objectId": result.object_id,
                "lifecycleStatus": result.lifecycle_status,
                "parsedDocumentId": result.parsed_document_id,
                "failureReason": result.failure_reason,
            }),
            Ok(None) => json!({
                "jobId": job_id,
                "status": "skipped",
            }),
            Err(error) => json!({
                "jobId": job_id,
                "status": "failed",
                "failureReason": error.to_string(),
            }),
        };

        let _ = app_handle.emit("capture://job-completed", payload);
        let _ = app_handle.emit("library://objects-updated", ());

        if let Some(object_id) = ai_object_id {
            spawn_ai_enrichment_runner(app_handle, ai_service, object_id);
        }
    });
}

#[derive(Debug)]
struct FetchedHtmlDocument {
    raw_html: String,
    title: Option<String>,
    author: Option<String>,
    text_content: String,
    markdown_content: String,
    language: Option<String>,
    word_count: i64,
    content_hash: String,
}

fn build_http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent(format!(
            "LinkWorld/{} local-first capture",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| AppError::Unknown(error.to_string()))
}

fn map_reqwest_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::NetworkTimeout
    } else if error.is_connect() {
        AppError::ParseFailed("network connection failed".to_string())
    } else if error.is_decode() {
        AppError::ParseFailed("response body could not be decoded".to_string())
    } else {
        AppError::ParseFailed("network request failed".to_string())
    }
}

fn capture_failure_reason(error: &AppError) -> String {
    match error {
        AppError::NetworkTimeout => format!(
            "capture.timeout: URL fetch timed out after {FETCH_TIMEOUT_SECONDS} seconds. Retry later, or if the page loads in your browser, save it with the browser extension."
        ),
        AppError::ParseFailed(message) => capture_parse_failure_reason(message),
        AppError::PolicyDenied(message) => capture_policy_failure_reason(message),
        _ => "capture.failed: Capture failed before parsing could complete. Retry the capture or use the browser extension.".to_string(),
    }
}

fn capture_parse_failure_reason(message: &str) -> String {
    if let Some(status_code) = parse_http_status_code(message) {
        return capture_http_status_failure_reason(status_code);
    }

    let lower_message = message.to_lowercase();
    if lower_message.contains("verification page")
        || lower_message.contains("captcha")
        || lower_message.contains("challenge")
    {
        return "capture.restricted_page: The fetched page appears to be a login, CAPTCHA, anti-bot, or environment verification page. Open it in your browser and save it with the browser extension after access is confirmed.".to_string();
    }

    if lower_message.contains("no readable text")
        || lower_message.contains("did not contain readable text")
    {
        return capture_no_readable_text_failure_reason();
    }

    if lower_message.contains("network connection failed") {
        return "capture.network_unreachable: Link World could not connect to the host. Check network, DNS, VPN, or proxy settings, then retry.".to_string();
    }

    if lower_message.contains("could not be decoded") {
        return "capture.invalid_response: The server response could not be decoded as readable HTML. Try the browser extension capture path or save selected text manually.".to_string();
    }

    "capture.parse_failed: Link World could not extract readable content from the fetched page. Try the browser extension capture path, selected text capture, or a different source URL.".to_string()
}

fn capture_policy_failure_reason(message: &str) -> String {
    if message.contains("unsupported URL scheme") {
        return "capture.unsupported_scheme: Only http and https URLs can be fetched automatically. Save local files, app links, or other schemes through import or selected text capture.".to_string();
    }

    if message.contains("fetched HTML exceeds") {
        return "capture.too_large: The fetched page is larger than the capture safety limit. Use selected text or browser extension capture for the relevant section.".to_string();
    }

    "capture.policy_denied: Capture was blocked by a safety policy. Retry with a user-confirmed URL, selected text, or browser extension capture.".to_string()
}

fn capture_http_status_failure_reason(status_code: u16) -> String {
    match status_code {
        401 | 403 => format!(
            "capture.http_forbidden: The server returned HTTP {status_code}, so Link World cannot fetch the page without browser/session access. Open it in your browser and save it with the browser extension."
        ),
        404 | 410 => format!(
            "capture.http_not_found: The server returned HTTP {status_code}. Check whether the URL is still valid, or save a browser-visible copy if you can access it."
        ),
        408 | 425 | 429 => format!(
            "capture.http_retryable: The server returned HTTP {status_code}. Wait and retry; if the page is visible in your browser, use the browser extension capture path."
        ),
        500..=599 => format!(
            "capture.http_server_error: The server returned HTTP {status_code}. Retry later; the saved object remains available for retry."
        ),
        _ => format!(
            "capture.http_error: The server returned HTTP {status_code}. Retry later or use the browser extension capture path if the page is visible in your browser."
        ),
    }
}

fn parse_http_status_code(message: &str) -> Option<u16> {
    let marker = "URL returned HTTP ";
    let after_marker = message.split_once(marker)?.1;
    let digits = after_marker
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn capture_no_readable_text_failure_reason() -> String {
    "capture.no_readable_text: The fetched page did not contain enough readable article text. Try the browser extension capture path, selected text capture, or a more direct article URL.".to_string()
}

async fn parse_fetched_html(html: String) -> AppResult<FetchedHtmlDocument> {
    tokio::task::spawn_blocking(move || parse_fetched_html_sync(html))
        .await
        .map_err(|error| AppError::ParseFailed(error.to_string()))?
}

fn parse_fetched_html_sync(html: String) -> AppResult<FetchedHtmlDocument> {
    let parsed = parse_html_document(&html, DocumentHints::default())?;
    let word_count = parsed.text_content.split_whitespace().count() as i64;
    let content_hash = sha256_hex(parsed.text_content.as_bytes());

    Ok(FetchedHtmlDocument {
        raw_html: html,
        title: parsed.title,
        author: parsed.author,
        text_content: parsed.text_content,
        markdown_content: parsed.markdown_content,
        language: parsed.language,
        word_count,
        content_hash,
    })
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

fn is_deduplicated_url_capture(item: &RawCaptureItem) -> bool {
    item.source_type == "url" && !has_text(&item.raw_text) && !has_text(&item.raw_html)
}

fn normalized_capture_canonical_url(item: &RawCaptureItem) -> Option<String> {
    item.canonical_url
        .as_deref()
        .or(item.source_url.as_deref())
        .and_then(normalize_capture_url)
}

fn normalize_capture_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let Ok(mut url) = Url::parse(trimmed) else {
        return Some(trimmed.to_string());
    };

    url.set_fragment(None);
    if matches!(
        (url.scheme(), url.port()),
        ("http", Some(80)) | ("https", Some(443))
    ) {
        let _ = url.set_port(None);
    }

    Some(url.to_string())
}

fn existing_capture_response(existing: ExistingCaptureRecord) -> SubmitCaptureResponse {
    SubmitCaptureResponse {
        object_id: existing.object_id,
        snapshot_id: existing.snapshot_id,
        parsed_document_id: existing.parsed_document_id,
        job_id: existing.job_id,
        deduplicated: true,
    }
}

fn build_inline_parsed_document(
    item: &RawCaptureItem,
    created_at: &str,
) -> AppResult<Option<CaptureParsedDocumentSubmission>> {
    if item.source_type != "selection" {
        if let Some(html) = item
            .raw_html
            .as_deref()
            .filter(|html| !html.trim().is_empty())
        {
            let parsed = parse_html_document(html, document_hints(item))?;
            return Ok(Some(web_document_submission(parsed, created_at)));
        }
    }

    let Some(text_content) = item
        .raw_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
    else {
        return Ok(None);
    };
    let content_hash = sha256_hex(text_content.as_bytes());
    let word_count = text_content.split_whitespace().count() as i64;

    Ok(Some(CaptureParsedDocumentSubmission {
        id: Uuid::new_v4().to_string(),
        title: normalized_title(item),
        markdown_content: Some(text_content.clone()),
        language: metadata_string(item, "language"),
        text_content,
        word_count,
        content_hash,
        parser_id: INLINE_CAPTURE_PARSER_ID.to_string(),
        parser_version: INLINE_CAPTURE_PARSER_VERSION.to_string(),
        created_at: created_at.to_string(),
    }))
}

fn web_document_submission(
    parsed: ParsedWebDocument,
    created_at: &str,
) -> CaptureParsedDocumentSubmission {
    let content_hash = sha256_hex(parsed.text_content.as_bytes());
    let word_count = parsed.text_content.split_whitespace().count() as i64;
    CaptureParsedDocumentSubmission {
        id: Uuid::new_v4().to_string(),
        title: parsed.title,
        text_content: parsed.text_content,
        markdown_content: Some(parsed.markdown_content),
        language: parsed.language,
        word_count,
        content_hash,
        parser_id: HTML_FETCH_PARSER_ID.to_string(),
        parser_version: HTML_FETCH_PARSER_VERSION.to_string(),
        created_at: created_at.to_string(),
    }
}

fn document_hints(item: &RawCaptureItem) -> DocumentHints {
    DocumentHints {
        title: normalized_title(item),
        author: item.author.clone(),
        description: metadata_string(item, "description"),
        language: metadata_string(item, "language"),
    }
}

fn metadata_string(item: &RawCaptureItem, key: &str) -> Option<String> {
    item.metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalized_title(item: &RawCaptureItem) -> Option<String> {
    item.title
        .as_ref()
        .map(|title| title.trim())
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
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

fn build_job_payload(object_id: &str, snapshot_id: &str, correlation_id: &str) -> String {
    json!({
        "objectId": object_id,
        "snapshotId": snapshot_id,
        "correlationId": correlation_id,
    })
    .to_string()
}

fn build_domain_events(
    item: &RawCaptureItem,
    object_id: &str,
    snapshot_id: &str,
    correlation_id: &str,
    occurred_at: &str,
) -> Vec<CaptureDomainEventSubmission> {
    let user_id = item
        .user_id
        .clone()
        .unwrap_or_else(|| LOCAL_USER_ID.to_string());
    let mut events = Vec::with_capacity(3);

    events.push(CaptureDomainEventSubmission {
        id: Uuid::new_v4().to_string(),
        event_type: "capture.submitted".to_string(),
        event_version: 1,
        user_id: user_id.clone(),
        correlation_id: correlation_id.to_string(),
        payload_json: json!({
            "objectId": object_id,
            "sourceType": item.source_type,
            "sourcePlatform": item.source_platform,
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
        correlation_id: correlation_id.to_string(),
        payload_json: json!({
            "snapshotId": snapshot_id,
        })
        .to_string(),
        occurred_at: occurred_at.to_string(),
    });

    if has_text(&item.raw_text) || has_text(&item.raw_html) {
        let parser_id = if item.source_type != "selection" && has_text(&item.raw_html) {
            HTML_FETCH_PARSER_ID
        } else {
            INLINE_CAPTURE_PARSER_ID
        };
        events.push(CaptureDomainEventSubmission {
            id: Uuid::new_v4().to_string(),
            event_type: "object.parsed".to_string(),
            event_version: 1,
            user_id,
            correlation_id: correlation_id.to_string(),
            payload_json: json!({
                "objectId": object_id,
                "parserId": parser_id,
            })
            .to_string(),
            occurred_at: occurred_at.to_string(),
        });
    }

    events
}

fn build_fetch_success_events(
    job: &CaptureFetchJobRecord,
    snapshot_id: &str,
    parsed_document_id: &str,
    occurred_at: &str,
) -> Vec<CaptureDomainEventSubmission> {
    vec![
        CaptureDomainEventSubmission {
            id: Uuid::new_v4().to_string(),
            event_type: "snapshot.created".to_string(),
            event_version: 1,
            user_id: job.user_id.clone(),
            correlation_id: job.correlation_id.clone(),
            payload_json: json!({
                "snapshotId": snapshot_id,
                "source": "capture.fetch_url",
            })
            .to_string(),
            occurred_at: occurred_at.to_string(),
        },
        CaptureDomainEventSubmission {
            id: Uuid::new_v4().to_string(),
            event_type: "object.parsed".to_string(),
            event_version: 1,
            user_id: job.user_id.clone(),
            correlation_id: job.correlation_id.clone(),
            payload_json: json!({
                "objectId": job.object_id,
                "parsedDocumentId": parsed_document_id,
                "parserId": HTML_FETCH_PARSER_ID,
            })
            .to_string(),
            occurred_at: occurred_at.to_string(),
        },
    ]
}

fn build_fetch_failed_event(
    job: &CaptureFetchJobRecord,
    failure_reason: &str,
    occurred_at: &str,
) -> CaptureDomainEventSubmission {
    CaptureDomainEventSubmission {
        id: Uuid::new_v4().to_string(),
        event_type: "object.failed".to_string(),
        event_version: 1,
        user_id: job.user_id.clone(),
        correlation_id: job.correlation_id.clone(),
        payload_json: json!({
            "objectId": job.object_id,
            "jobId": job.id,
            "reason": failure_reason,
        })
        .to_string(),
        occurred_at: occurred_at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_domain_events, build_inline_parsed_document, capture_failure_reason,
        parse_fetched_html_sync, CaptureService, HTML_FETCH_PARSER_ID,
    };
    use crate::domain::capture::{PermissionContext, RawCaptureItem};
    use crate::errors::AppError;
    use crate::repositories::search::SearchRepository;
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn parse_fetched_html_prefers_readable_content_over_script_and_style_noise() {
        let parsed = parse_fetched_html_sync(
            r#"<!doctype html>
            <html lang="zh-CN">
              <head>
                <title>Readable Guide</title>
                <style>:root { --weui-BG-0: #ededed; color: rgba(0,0,0,.9); }</style>
              </head>
              <body>
                <script>
                  document.addEventListener('DOMContentLoaded', function() {
                    window.localStorage.setItem('noise', 'true');
                    document.querySelectorAll('[data-pro-link]').forEach(function(item) {});
                  });
                </script>
                <main>
                  <article>
                    <h1>Readable Guide</h1>
                    <p>Use a structured capture checklist before saving useful links.</p>
                    <p>Keep the source snapshot, parsed text, and review state separate.</p>
                  </article>
                </main>
              </body>
            </html>"#
                .to_string(),
        )
        .expect("readable article should parse");

        assert_eq!(parsed.title.as_deref(), Some("Readable Guide"));
        assert!(parsed.text_content.contains("structured capture checklist"));
        assert!(!parsed.text_content.contains("document.addEventListener"));
        assert!(!parsed.text_content.contains("--weui"));
    }

    #[test]
    fn capture_failure_reason_classifies_timeout_and_empty_content() {
        let timeout = capture_failure_reason(&AppError::NetworkTimeout);
        assert!(timeout.contains("capture.timeout"));
        assert!(timeout.contains("browser extension"));

        let empty = capture_failure_reason(&AppError::ParseFailed(
            "fetched HTML did not contain readable text".to_string(),
        ));
        assert!(empty.contains("capture.no_readable_text"));
        assert!(empty.contains("selected text capture"));
    }

    #[test]
    fn capture_failure_reason_never_exposes_raw_error_details() {
        let sensitive_markers = [
            "raw-third-party-response-body",
            "cookie=session-secret",
            "token=provider-secret",
        ];
        let failures = [
            capture_failure_reason(&AppError::ParseFailed(
                "URL returned HTTP 502 raw-third-party-response-body cookie=session-secret"
                    .to_string(),
            )),
            capture_failure_reason(&AppError::ParseFailed(
                "network connection failed: token=provider-secret".to_string(),
            )),
            capture_failure_reason(&AppError::PolicyDenied(
                "unexpected policy input raw-third-party-response-body cookie=session-secret"
                    .to_string(),
            )),
            capture_failure_reason(&AppError::Unknown(
                "upstream failed: token=provider-secret raw-third-party-response-body".to_string(),
            )),
        ];

        assert!(failures[0].starts_with("capture.http_server_error:"));
        assert!(failures[1].starts_with("capture.network_unreachable:"));
        assert!(failures[2].starts_with("capture.policy_denied:"));
        assert!(failures[3].starts_with("capture.failed:"));
        for failure in failures {
            for sensitive_marker in sensitive_markers {
                assert!(!failure.contains(sensitive_marker));
            }
        }
    }

    #[test]
    fn capture_domain_events_share_correlation_without_copying_source_urls() {
        let item = RawCaptureItem {
            id: None,
            user_id: None,
            source_type: "url".to_string(),
            source_platform: Some("web".to_string()),
            source_url: Some(
                "https://example.com/article?token=query-secret#private-fragment".to_string(),
            ),
            canonical_url: Some(
                "https://example.com/article?token=query-secret#private-fragment".to_string(),
            ),
            title: None,
            author: None,
            captured_at: None,
            raw_html: None,
            raw_text: None,
            assets: Vec::new(),
            metadata: json!({}),
            privacy_level: "personal".to_string(),
            permission_context: confirmed_permission(),
        };
        let correlation_id = "d4b258f0-17cf-4b85-81f1-892ad3f10b27";
        let events = build_domain_events(
            &item,
            "object-1",
            "snapshot-1",
            correlation_id,
            "2026-06-29T00:00:00Z",
        );

        assert_eq!(events.len(), 2);
        for event in events {
            assert_eq!(event.correlation_id, correlation_id);
            assert!(!event.payload_json.contains("query-secret"));
            assert!(!event.payload_json.contains("private-fragment"));
            assert!(!event.payload_json.contains("example.com"));
            assert!(!event.payload_json.contains("sourceUrl"));
            assert!(!event.payload_json.contains("canonicalUrl"));
        }
    }

    #[test]
    fn parse_fetched_html_prefers_schema_article_body_and_preserves_block_structure() {
        let parsed = parse_fetched_html_sync(
            r#"<!doctype html>
            <html lang="zh-CN">
              <head>
                <title>Clean title plus page suffix and teaser - Example</title>
                <meta itemprop="headline" content="Clean title">
                <meta name="description" content="A compact article description.">
              </head>
              <body>
                <article>
                  <div itemprop="author"><meta itemprop="name" content="Example Author"></div>
                  <h1>Clean title</h1>
                  <div class="author-info"><p>42 reads and unrelated collection metadata</p></div>
                  <div itemprop="articleBody">
                    <blockquote><p>Boundary-aware intro.</p></blockquote>
                    <h2>First section</h2>
                    <p>Paragraph with <code>inline_code()</code>.</p>
                    <pre><code>const answer = 42;
return answer;</code></pre>
                    <table>
                      <tr><th>Capability</th><th>Result</th></tr>
                      <tr><td>Structure</td><td>Preserved</td></tr>
                    </table>
                  </div>
                </article>
              </body>
            </html>"#
                .to_string(),
        )
        .expect("schema article should parse");

        assert_eq!(parsed.title.as_deref(), Some("Clean title"));
        assert_eq!(parsed.author.as_deref(), Some("Example Author"));
        assert_eq!(parsed.language.as_deref(), Some("zh-CN"));
        assert!(!parsed.text_content.contains("42 reads"));
        assert_eq!(
            parsed.text_content.matches("Boundary-aware intro.").count(),
            1
        );
        assert_eq!(parsed.text_content.matches("const answer = 42;").count(), 1);
        assert!(parsed
            .text_content
            .contains("Boundary-aware intro.\n\nFirst section"));
        assert!(parsed
            .text_content
            .contains("const answer = 42;\nreturn answer;"));
        assert!(parsed
            .text_content
            .contains("Capability | Result\nStructure | Preserved"));
    }

    #[test]
    fn parse_fetched_html_rejects_verification_page() {
        let error = parse_fetched_html_sync(
            r#"<!doctype html>
            <html>
              <head><title>环境异常</title></head>
              <body>
                <div>环境异常 当前环境异常，完成验证后即可继续访问。去验证</div>
              </body>
            </html>"#
                .to_string(),
        )
        .expect_err("verification page should not be parsed as content");

        assert!(error.to_string().contains("verification page"));
    }

    #[test]
    fn inline_dom_capture_keeps_structured_markdown_and_language() {
        let item = RawCaptureItem {
            id: None,
            user_id: None,
            source_type: "dom".to_string(),
            source_platform: Some("example.com".to_string()),
            source_url: Some("https://example.com/post".to_string()),
            canonical_url: Some("https://example.com/post".to_string()),
            title: Some("Structured article".to_string()),
            author: Some("Author".to_string()),
            captured_at: None,
            raw_html: Some(
                "<div itemprop=\"articleBody\"><h2>Structured section</h2><p>Captured content keeps its semantic structure.</p><pre><code class=\"language-js\">let value = 1;</code></pre></div>"
                    .to_string(),
            ),
            raw_text: Some(
                "Structured section\n\nCaptured content keeps its semantic structure.\n\nlet value = 1;"
                    .to_string(),
            ),
            assets: Vec::new(),
            metadata: json!({
                "objectType": "article",
                "language": "zh-CN"
            }),
            privacy_level: "personal".to_string(),
            permission_context: confirmed_permission(),
        };

        let parsed = build_inline_parsed_document(&item, "2026-06-22T00:00:00Z")
            .expect("inline capture should parse")
            .expect("inline capture should produce a document");

        assert_eq!(parsed.language.as_deref(), Some("zh-CN"));
        assert_eq!(
            parsed.markdown_content.as_deref(),
            Some(
                "## Structured section\n\nCaptured content keeps its semantic structure.\n\n```js\nlet value = 1;\n```"
            )
        );
        assert_eq!(
            parsed.text_content,
            "Structured section\n\nCaptured content keeps its semantic structure.\n\nlet value = 1;"
        );
        assert_eq!(parsed.parser_id, HTML_FETCH_PARSER_ID);
    }

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

        let correlation_ids: Vec<String> = sqlx::query_scalar(
            "SELECT correlation_id FROM domain_events WHERE object_id = ?1 ORDER BY occurred_at, id",
        )
        .bind(&response.object_id)
        .fetch_all(database.pool())
        .await
        .expect("event correlation ids should be readable");
        let job_payload: String =
            sqlx::query_scalar("SELECT payload_json FROM background_jobs WHERE object_id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("job payload should be readable");
        let job_correlation_id = serde_json::from_str::<serde_json::Value>(&job_payload)
            .expect("job payload should be valid JSON")
            .get("correlationId")
            .and_then(serde_json::Value::as_str)
            .expect("job payload should carry correlation id")
            .to_string();

        assert!(uuid::Uuid::parse_str(&job_correlation_id).is_ok());
        assert_eq!(correlation_ids.len(), 3);
        assert!(correlation_ids
            .iter()
            .all(|correlation_id| correlation_id == &job_correlation_id));

        let search_results = SearchRepository::new(database.pool().clone())
            .search_hybrid("useful content", Some(10), None)
            .await
            .expect("inline parsed capture should be searchable");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].object.id, response.object_id);
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

    #[tokio::test]
    async fn submit_url_capture_deduplicates_normalized_canonical_url() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);

        let first = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some("https://Example.com:443/article#comments".to_string()),
                canonical_url: None,
                title: None,
                author: None,
                captured_at: Some("2026-06-16T00:00:00Z".to_string()),
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("first capture should be submitted");
        let first_job_id = first
            .job_id
            .clone()
            .expect("first URL capture should create a fetch job");

        let second = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some("https://example.com/article".to_string()),
                canonical_url: Some("https://example.com/article#later".to_string()),
                title: None,
                author: None,
                captured_at: Some("2026-06-16T00:01:00Z".to_string()),
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("duplicate capture should return existing object");

        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        assert_eq!(second.object_id, first.object_id);
        assert_eq!(second.job_id.as_deref(), Some(first_job_id.as_str()));

        let object_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_objects")
            .fetch_one(database.pool())
            .await
            .expect("object count should be readable");
        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM source_snapshots")
            .fetch_one(database.pool())
            .await
            .expect("snapshot count should be readable");
        let job_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM background_jobs")
            .fetch_one(database.pool())
            .await
            .expect("job count should be readable");
        let canonical_url: String =
            sqlx::query_scalar("SELECT canonical_url FROM knowledge_objects WHERE id = ?1")
                .bind(&first.object_id)
                .fetch_one(database.pool())
                .await
                .expect("canonical URL should be readable");

        assert_eq!(object_count, 1);
        assert_eq!(snapshot_count, 1);
        assert_eq!(job_count, 1);
        assert_eq!(canonical_url, "https://example.com/article");
    }

    #[tokio::test]
    async fn run_fetch_job_fetches_html_and_marks_object_parsed() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);
        let url = start_test_html_server(
            r#"<!doctype html>
            <html lang="en">
              <head>
                <title>Fetched Article</title>
                <meta name="author" content="Fetch Author">
                <meta name="description" content="A useful article.">
              </head>
              <body><main><h1>Fetched Article</h1><p>This page was fetched by the local job runner.</p></main></body>
            </html>"#,
        )
        .await;

        let response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some(url.clone()),
                canonical_url: Some(url),
                title: None,
                author: None,
                captured_at: Some("2026-06-16T00:00:00Z".to_string()),
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("capture should be submitted");

        assert!(response.parsed_document_id.is_none());
        let job_id = response
            .job_id
            .clone()
            .expect("new URL capture should create a fetch job");

        let run_result = service
            .run_fetch_job(&job_id)
            .await
            .expect("job runner should not error")
            .expect("job should be claimed");

        assert_eq!(run_result.status, "succeeded");
        assert_eq!(run_result.lifecycle_status, "parsed");
        assert!(run_result.parsed_document_id.is_some());

        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("object status should be readable");
        let title: String = sqlx::query_scalar("SELECT title FROM knowledge_objects WHERE id = ?1")
            .bind(&response.object_id)
            .fetch_one(database.pool())
            .await
            .expect("object title should be readable");
        let author: String =
            sqlx::query_scalar("SELECT author FROM knowledge_objects WHERE id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("object author should be readable");
        let job_status: String =
            sqlx::query_scalar("SELECT status FROM background_jobs WHERE id = ?1")
                .bind(&job_id)
                .fetch_one(database.pool())
                .await
                .expect("job status should be readable");
        let snapshot_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM source_snapshots WHERE object_id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("snapshot count should be readable");
        let parsed_text: String =
            sqlx::query_scalar("SELECT text_content FROM parsed_documents WHERE object_id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("parsed text should be readable");

        assert_eq!(lifecycle_status, "parsed");
        assert_eq!(title, "Fetched Article");
        assert_eq!(author, "Fetch Author");
        assert_eq!(job_status, "succeeded");
        assert_eq!(snapshot_count, 2);
        assert!(parsed_text.contains("local job runner"));

        let correlation_ids: Vec<String> = sqlx::query_scalar(
            "SELECT correlation_id FROM domain_events WHERE object_id = ?1 ORDER BY occurred_at, id",
        )
        .bind(&response.object_id)
        .fetch_all(database.pool())
        .await
        .expect("fetch lifecycle correlation ids should be readable");
        assert_eq!(correlation_ids.len(), 4);
        assert!(uuid::Uuid::parse_str(&correlation_ids[0]).is_ok());
        assert!(correlation_ids
            .iter()
            .all(|correlation_id| correlation_id == &correlation_ids[0]));

        let search_results = SearchRepository::new(database.pool().clone())
            .search_hybrid("local job runner", Some(10), None)
            .await
            .expect("fetched parsed content should be searchable");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].object.id, response.object_id);
    }

    #[tokio::test]
    async fn run_fetch_job_marks_object_failed_for_verification_page() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);
        let url = start_test_html_server(
            r#"<!doctype html>
            <html>
              <head><title>环境异常</title></head>
              <body>
                <style>:root { --weui-BG-0:#ededed; --weui-FG-0:rgba(0,0,0,.9); }</style>
                <div>环境异常 当前环境异常，完成验证后即可继续访问。去验证</div>
              </body>
            </html>"#,
        )
        .await;

        let response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("wechat".to_string()),
                source_url: Some(url.clone()),
                canonical_url: Some(url),
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("capture should be submitted");
        let job_id = response
            .job_id
            .clone()
            .expect("new URL capture should create a fetch job");

        let run_result = service
            .run_fetch_job(&job_id)
            .await
            .expect("job runner should record failure")
            .expect("job should be claimed");

        assert_eq!(run_result.status, "failed");
        assert_eq!(run_result.lifecycle_status, "failed");
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("capture.restricted_page"));
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("browser extension"));

        let parsed_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM parsed_documents WHERE object_id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("parsed document count should be readable");
        let distinct_correlations: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT correlation_id) FROM domain_events WHERE object_id = ?1",
        )
        .bind(&response.object_id)
        .fetch_one(database.pool())
        .await
        .expect("failed fetch correlation count should be readable");

        assert_eq!(parsed_count, 0);
        assert_eq!(distinct_correlations, 1);
    }

    #[tokio::test]
    async fn run_fetch_job_marks_object_failed_for_unsupported_scheme() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);

        let response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: None,
                source_url: Some("ftp://example.com/archive".to_string()),
                canonical_url: Some("ftp://example.com/archive".to_string()),
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("capture should be submitted");
        let job_id = response
            .job_id
            .clone()
            .expect("new URL capture should create a fetch job");

        let run_result = service
            .run_fetch_job(&job_id)
            .await
            .expect("job runner should record failure")
            .expect("job should be claimed");

        assert_eq!(run_result.status, "failed");
        assert_eq!(run_result.lifecycle_status, "failed");
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("capture.unsupported_scheme"));
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("selected text capture"));

        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("object status should be readable");
        let job_status: String =
            sqlx::query_scalar("SELECT status FROM background_jobs WHERE id = ?1")
                .bind(&job_id)
                .fetch_one(database.pool())
                .await
                .expect("job status should be readable");

        assert_eq!(lifecycle_status, "failed");
        assert_eq!(job_status, "failed");
    }

    #[tokio::test]
    async fn run_fetch_job_marks_restricted_http_status_with_extension_fallback() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);
        let url = start_test_http_server(
            "403 Forbidden",
            r#"<!doctype html><html><body>Forbidden</body></html>"#,
        )
        .await;

        let response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some(url.clone()),
                canonical_url: Some(url),
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("capture should be submitted");
        let job_id = response
            .job_id
            .clone()
            .expect("new URL capture should create a fetch job");

        let run_result = service
            .run_fetch_job(&job_id)
            .await
            .expect("job runner should record failure")
            .expect("job should be claimed");

        assert_eq!(run_result.status, "failed");
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("capture.http_forbidden"));
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("browser extension"));
    }

    #[tokio::test]
    async fn failed_fetch_job_does_not_block_later_fetch_jobs() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let service = CaptureService::new(database.pool().clone(), object_store);
        let forbidden_url = start_test_http_server(
            "403 Forbidden",
            r#"<!doctype html><html><body>Forbidden</body></html>"#,
        )
        .await;
        let success_url = start_test_html_server(
            r#"<!doctype html>
            <html>
              <head><title>Independent Success</title></head>
              <body><main><h1>Independent Success</h1><p>The second capture continues after the first job fails.</p></main></body>
            </html>"#,
        )
        .await;

        let failed_response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some(forbidden_url.clone()),
                canonical_url: Some(forbidden_url),
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("failed capture should be submitted");
        let success_response = service
            .submit(RawCaptureItem {
                id: None,
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some(success_url.clone()),
                canonical_url: Some(success_url),
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: "personal".to_string(),
                permission_context: confirmed_permission(),
            })
            .await
            .expect("second capture should be submitted");

        let failed_job_id = failed_response
            .job_id
            .clone()
            .expect("failed URL capture should create a fetch job");
        let success_job_id = success_response
            .job_id
            .clone()
            .expect("second URL capture should create a fetch job");

        let failed_run = service
            .run_fetch_job(&failed_job_id)
            .await
            .expect("failed job should be recorded")
            .expect("failed job should be claimed");
        assert_eq!(failed_run.status, "failed");
        assert!(failed_run
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("capture.http_forbidden"));

        let success_run = service
            .run_fetch_job(&success_job_id)
            .await
            .expect("second job should still run")
            .expect("second job should be claimed");
        assert_eq!(success_run.status, "succeeded");
        assert_eq!(success_run.lifecycle_status, "parsed");

        let failed_lifecycle: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&failed_response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("failed object lifecycle should be readable");
        let success_lifecycle: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&success_response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("success object lifecycle should be readable");
        let success_text: String =
            sqlx::query_scalar("SELECT text_content FROM parsed_documents WHERE object_id = ?1")
                .bind(&success_response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("success parsed document should be readable");

        assert_eq!(failed_lifecycle, "failed");
        assert_eq!(success_lifecycle, "parsed");
        assert!(success_text.contains("continues after the first job fails"));
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

    async fn start_test_html_server(body: &'static str) -> String {
        start_test_http_server("200 OK", body).await
    }

    async fn start_test_http_server(status: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let addr = listener
            .local_addr()
            .expect("test server address should be readable");

        tokio::spawn(async move {
            let (mut socket, _) = listener
                .accept()
                .await
                .expect("test server should accept one request");
            let mut buffer = [0_u8; 1024];
            let _ = socket.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("test server should write response");
        });

        format!("http://{addr}/article")
    }
}
