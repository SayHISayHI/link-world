use crate::domain::capture::{
    CaptureBackgroundJobSubmission, CaptureDomainEventSubmission, CaptureFetchJobRecord,
    CaptureFetchJobRunResult, CaptureParsedDocumentSubmission, CaptureSnapshotSubmission,
    CaptureSubmission, RawCaptureItem, SubmitCaptureResponse,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::capture::CaptureRepository;
use crate::services::ai::{spawn_ai_enrichment_runner, AIEnrichmentService};
use crate::state::AppState;
use crate::storage::object_store::{sha256_hex, ObjectStore};
use chrono::Utc;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
use serde_json::json;
use sqlx::SqlitePool;
use std::time::Duration;
use tauri::Emitter;
use uuid::Uuid;

const LOCAL_USER_ID: &str = "local";
const INLINE_CAPTURE_PARSER_ID: &str = "builtin.inline_capture_parser";
const INLINE_CAPTURE_PARSER_VERSION: &str = "0.1.0";
const HTML_FETCH_PARSER_ID: &str = "builtin.html_fetch_parser";
const HTML_FETCH_PARSER_VERSION: &str = "0.1.0";
const MAX_RAW_CAPTURE_BYTES: usize = 5 * 1024 * 1024;
const MAX_FETCH_BYTES: usize = 5 * 1024 * 1024;
const FETCH_TIMEOUT_SECONDS: u64 = 20;
const MIN_EXTRACTED_TEXT_CHARS: usize = 24;
const READABLE_TEXT_BLOCK_SELECTOR: &str =
    "h1, h2, h3, h4, p, li, blockquote, pre, code, figcaption";
const READABLE_CONTAINER_SELECTORS: &[&str] = &[
    "article",
    "main",
    r#"[role="main"]"#,
    r#"[itemprop="articleBody"]"#,
    "#js_content",
    ".rich_media_content",
    ".topic_content",
    ".markdown_body",
    ".entry-content",
    ".article-content",
    ".post-content",
    ".Post-RichText",
    ".RichContent-inner",
    r#"[data-testid="tweetText"]"#,
    ".article",
    ".post",
    ".content",
    "body",
];

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

        let object_id = Uuid::new_v4().to_string();
        let snapshot_id = Uuid::new_v4().to_string();
        let job_id = Uuid::new_v4().to_string();
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
            parser_id: parsed
                .as_ref()
                .map(|_| INLINE_CAPTURE_PARSER_ID.to_string()),
            parser_version: parsed
                .as_ref()
                .map(|_| INLINE_CAPTURE_PARSER_VERSION.to_string()),
            captured_at: now.clone(),
        };

        let submission = CaptureSubmission {
            object_id: object_id.clone(),
            object_type: infer_object_type(&item),
            user_id: item
                .user_id
                .clone()
                .unwrap_or_else(|| LOCAL_USER_ID.to_string()),
            title: normalized_title(&item),
            canonical_url: item
                .canonical_url
                .clone()
                .or_else(|| item.source_url.clone()),
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
            Err(error) => self.fail_fetch_job(job, error.to_string()).await.map(Some),
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
                .fail_fetch_job(
                    job,
                    "fetched HTML did not contain readable text".to_string(),
                )
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
            &job.id,
            &job.object_id,
            &job.user_id,
            parsed.title.as_deref(),
            &snapshot,
            &parsed_document,
            &events,
            &now,
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
    } else {
        AppError::ParseFailed(error.to_string())
    }
}

async fn parse_fetched_html(html: String) -> AppResult<FetchedHtmlDocument> {
    tokio::task::spawn_blocking(move || parse_fetched_html_sync(html))
        .await
        .map_err(|error| AppError::ParseFailed(error.to_string()))?
}

fn parse_fetched_html_sync(html: String) -> AppResult<FetchedHtmlDocument> {
    let document = Html::parse_document(&html);
    let title = select_first_text(&document, "title");
    let description = select_first_attr(&document, r#"meta[name="description"]"#, "content");
    let language = select_first_attr(&document, "html", "lang");
    let text_content = select_readable_text(&document)
        .or_else(|| description.clone())
        .unwrap_or_else(|| strip_html_tags(&html));
    let text_content = normalize_whitespace(&text_content);

    if text_content.is_empty() {
        return Err(AppError::ParseFailed(
            "HTML document did not contain readable text".to_string(),
        ));
    }

    if let Some(reason) =
        detect_blocked_or_verification_page(title.as_deref(), description.as_deref(), &text_content)
    {
        return Err(AppError::ParseFailed(reason));
    }

    if !is_meaningful_extracted_text(&text_content, description.as_deref()) {
        return Err(AppError::ParseFailed(
            "HTML document did not contain enough readable content".to_string(),
        ));
    }

    if looks_like_script_or_style_dump(&text_content) {
        return Err(AppError::ParseFailed(
            "HTML parser extracted script/style noise instead of readable content".to_string(),
        ));
    }

    let markdown_content =
        build_markdown_preview(title.as_deref(), description.as_deref(), &text_content);
    let word_count = text_content.split_whitespace().count() as i64;
    let content_hash = sha256_hex(text_content.as_bytes());

    Ok(FetchedHtmlDocument {
        raw_html: html,
        title,
        text_content,
        markdown_content,
        language,
        word_count,
        content_hash,
    })
}

fn select_first_text(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .map(|element| element.text().collect::<Vec<_>>().join(" "))
        .map(|text| normalize_whitespace(&text))
        .filter(|text| !text.is_empty())
}

fn select_first_attr(document: &Html, selector: &str, attr: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|element| element.value().attr(attr))
        .map(normalize_whitespace)
        .filter(|text| !text.is_empty())
}

fn select_readable_text(document: &Html) -> Option<String> {
    let mut best: Option<(usize, String)> = None;

    for selector_text in READABLE_CONTAINER_SELECTORS {
        let Ok(selector) = Selector::parse(selector_text) else {
            continue;
        };

        for element in document.select(&selector) {
            let allow_fallback = *selector_text != "body";
            let Some(candidate) = collect_candidate_text(element, allow_fallback) else {
                continue;
            };
            let score = readable_text_score(&candidate, allow_fallback);
            let should_replace = match best.as_ref() {
                Some((best_score, _)) => score > *best_score,
                None => true,
            };

            if should_replace {
                best = Some((score, candidate));
            }
        }
    }

    best.map(|(_, text)| text)
}

fn collect_candidate_text(element: ElementRef<'_>, allow_fallback: bool) -> Option<String> {
    let block_selector = Selector::parse(READABLE_TEXT_BLOCK_SELECTOR).ok()?;
    let mut blocks = Vec::new();

    for block in element.select(&block_selector) {
        let block_text = normalize_whitespace(&block.text().collect::<Vec<_>>().join(" "));
        if is_readable_text_block(&block_text) {
            blocks.push(block_text);
        }
    }

    let block_text = normalize_whitespace(&blocks.join("\n\n"));
    if is_readable_text_block(&block_text) {
        return Some(block_text);
    }

    if allow_fallback {
        let fallback = normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "));
        if is_readable_text_block(&fallback) {
            return Some(fallback);
        }
    }

    None
}

fn is_readable_text_block(text: &str) -> bool {
    text.chars().any(|character| !character.is_whitespace())
}

fn readable_text_score(text: &str, from_specific_container: bool) -> usize {
    let specificity_bonus = if from_specific_container { 10_000 } else { 0 };
    specificity_bonus
        + text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count()
}

fn is_meaningful_extracted_text(text: &str, description: Option<&str>) -> bool {
    let text_chars = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();

    if text_chars >= MIN_EXTRACTED_TEXT_CHARS {
        return true;
    }

    description
        .map(|description| {
            text_chars
                + description
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .count()
        })
        .is_some_and(|total_chars| total_chars >= MIN_EXTRACTED_TEXT_CHARS)
}

fn detect_blocked_or_verification_page(
    title: Option<&str>,
    description: Option<&str>,
    text_content: &str,
) -> Option<String> {
    let combined = format!(
        "{} {} {}",
        title.unwrap_or_default(),
        description.unwrap_or_default(),
        text_content
    );
    let lower_combined = combined.to_lowercase();

    let ascii_markers = [
        ("captcha", "captcha challenge"),
        ("verify you are human", "human verification"),
        ("security check", "security check"),
        ("access denied", "access denied"),
        ("checking your browser", "browser verification"),
        (
            "please enable javascript and cookies to continue",
            "browser verification",
        ),
    ];

    for (marker, reason) in ascii_markers {
        if lower_combined.contains(marker) {
            return Some(format!("blocked or verification page detected: {reason}"));
        }
    }

    let cjk_markers = [
        ("环境异常", "environment verification"),
        ("完成验证", "environment verification"),
        ("去验证", "environment verification"),
        ("安全验证", "security verification"),
        ("验证码", "captcha challenge"),
        ("访问受限", "access restricted"),
        ("请先登录", "login required"),
        ("登录后继续", "login required"),
    ];

    for (marker, reason) in cjk_markers {
        if combined.contains(marker) {
            return Some(format!("blocked or verification page detected: {reason}"));
        }
    }

    None
}

fn looks_like_script_or_style_dump(text: &str) -> bool {
    let lower_text = text.to_lowercase();

    if lower_text.matches("--weui-").count() >= 4 {
        return true;
    }

    let script_markers = [
        "document.",
        "addeventlistener",
        "queryselector",
        "window.",
        "function(",
        "function ",
        "var ",
        "__next_data__",
        "webpack",
    ];
    let style_markers = [
        "@media",
        "rgba(",
        "prefers-color-scheme",
        "background-",
        "font-",
        "color:",
    ];

    let script_hits = script_markers
        .iter()
        .filter(|marker| lower_text.contains(**marker))
        .count();
    let style_hits = style_markers
        .iter()
        .filter(|marker| lower_text.contains(**marker))
        .count();
    let non_whitespace_chars = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    let syntax_chars = text
        .chars()
        .filter(|character| matches!(character, '{' | '}' | '(' | ')' | ';' | '='))
        .count();
    let syntax_ratio = syntax_chars as f32 / non_whitespace_chars.max(1) as f32;

    (script_hits >= 4 && syntax_ratio > 0.06 && non_whitespace_chars > 300)
        || (style_hits >= 4 && lower_text.matches("--").count() >= 10)
}

fn build_markdown_preview(
    title: Option<&str>,
    description: Option<&str>,
    text_content: &str,
) -> String {
    let mut markdown = String::new();

    if let Some(title) = title {
        markdown.push_str("# ");
        markdown.push_str(title);
        markdown.push_str("\n\n");
    }

    if let Some(description) = description {
        markdown.push_str("> ");
        markdown.push_str(description);
        markdown.push_str("\n\n");
    }

    markdown.push_str(text_content);
    markdown
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    use super::{parse_fetched_html_sync, CaptureService};
    use crate::domain::capture::{PermissionContext, RawCaptureItem};
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
              <head><title>Fetched Article</title><meta name="description" content="A useful article."></head>
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

        let run_result = service
            .run_fetch_job(&response.job_id)
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
        let job_status: String =
            sqlx::query_scalar("SELECT status FROM background_jobs WHERE id = ?1")
                .bind(&response.job_id)
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
        assert_eq!(job_status, "succeeded");
        assert_eq!(snapshot_count, 2);
        assert!(parsed_text.contains("local job runner"));
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

        let run_result = service
            .run_fetch_job(&response.job_id)
            .await
            .expect("job runner should record failure")
            .expect("job should be claimed");

        assert_eq!(run_result.status, "failed");
        assert_eq!(run_result.lifecycle_status, "failed");
        assert!(run_result
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("verification page"));

        let parsed_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM parsed_documents WHERE object_id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("parsed document count should be readable");

        assert_eq!(parsed_count, 0);
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

        let run_result = service
            .run_fetch_job(&response.job_id)
            .await
            .expect("job runner should record failure")
            .expect("job should be claimed");

        assert_eq!(run_result.status, "failed");
        assert_eq!(run_result.lifecycle_status, "failed");

        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind(&response.object_id)
                .fetch_one(database.pool())
                .await
                .expect("object status should be readable");
        let job_status: String =
            sqlx::query_scalar("SELECT status FROM background_jobs WHERE id = ?1")
                .bind(&response.job_id)
                .fetch_one(database.pool())
                .await
                .expect("job status should be readable");

        assert_eq!(lifecycle_status, "failed");
        assert_eq!(job_status, "failed");
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
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
