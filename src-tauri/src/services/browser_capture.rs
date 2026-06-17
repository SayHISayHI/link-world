use crate::domain::capture::{PermissionContext, RawCaptureItem, SubmitCaptureResponse};
use crate::errors::{AppError, AppResult};
use crate::services::ai::{spawn_ai_enrichment_runner, AIEnrichmentService};
use crate::services::capture::{spawn_fetch_job_runner, CaptureService};
use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

const LOOPBACK_CAPTURE_ADDR: &str = "127.0.0.1:17321";
const CAPTURE_PATH: &str = "/capture";
const READ_TIMEOUT_SECONDS: u64 = 5;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 512 * 1024;
const MAX_RAW_TEXT_CHARS: usize = 80_000;
const MAX_RAW_HTML_CHARS: usize = 180_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserCapturePayload {
    url: String,
    title: Option<String>,
    selected_text: Option<String>,
    dom_text: Option<String>,
    dom_html: Option<String>,
    source_platform: Option<String>,
    captured_at: Option<String>,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HttpResponse {
    status_code: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
    cors_origin: Option<String>,
}

pub fn spawn_loopback_capture_server(
    app_handle: tauri::AppHandle,
    service: CaptureService,
    ai_service: AIEnrichmentService,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) =
            run_loopback_capture_server(app_handle.clone(), service, ai_service).await
        {
            let _ = app_handle.emit(
                "capture://browser-server-failed",
                json!({
                    "address": LOOPBACK_CAPTURE_ADDR,
                    "failureReason": error.to_string(),
                }),
            );
        }
    });
}

async fn run_loopback_capture_server(
    app_handle: tauri::AppHandle,
    service: CaptureService,
    ai_service: AIEnrichmentService,
) -> AppResult<()> {
    let listener = TcpListener::bind(LOOPBACK_CAPTURE_ADDR).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let connection_service = service.clone();
        let connection_ai_service = ai_service.clone();
        let connection_app_handle = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            handle_connection(
                socket,
                connection_app_handle,
                connection_service,
                connection_ai_service,
            )
            .await;
        });
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    app_handle: tauri::AppHandle,
    service: CaptureService,
    ai_service: AIEnrichmentService,
) {
    let response = match timeout(
        Duration::from_secs(READ_TIMEOUT_SECONDS),
        read_http_request(&mut socket),
    )
    .await
    {
        Ok(Ok(request)) => handle_http_request(request, app_handle, service, ai_service).await,
        Ok(Err(error)) => json_response(
            400,
            "Bad Request",
            json!({ "ok": false, "error": error.to_string() }),
            None,
        ),
        Err(_) => json_response(
            408,
            "Request Timeout",
            json!({ "ok": false, "error": "browser capture endpoint read timeout" }),
            None,
        ),
    };

    let _ = socket.write_all(&response.into_bytes()).await;
    let _ = socket.shutdown().await;
}

async fn handle_http_request(
    request: HttpRequest,
    app_handle: tauri::AppHandle,
    service: CaptureService,
    ai_service: AIEnrichmentService,
) -> HttpResponse {
    let cors_origin = allowed_cors_origin(&request);
    let path = request.path.split('?').next().unwrap_or_default();

    if request.method == "OPTIONS" && path == CAPTURE_PATH {
        return match cors_origin {
            Some(origin) => empty_response(204, "No Content", Some(origin)),
            None => json_response(
                403,
                "Forbidden",
                json!({ "ok": false, "error": "origin is not allowed" }),
                None,
            ),
        };
    }

    if request.method != "POST" || path != CAPTURE_PATH {
        return json_response(
            404,
            "Not Found",
            json!({ "ok": false, "error": "unknown browser capture endpoint" }),
            cors_origin,
        );
    }

    if !is_json_request(&request) {
        return json_response(
            415,
            "Unsupported Media Type",
            json!({ "ok": false, "error": "browser capture endpoint requires application/json" }),
            cors_origin,
        );
    }

    let result = submit_browser_capture(request.body, app_handle, service, ai_service).await;
    match result {
        Ok(response) => json_response(
            200,
            "OK",
            json!({
                "ok": true,
                "objectId": response.object_id,
                "snapshotId": response.snapshot_id,
                "parsedDocumentId": response.parsed_document_id,
                "jobId": response.job_id,
            }),
            cors_origin,
        ),
        Err(error) => json_response(
            400,
            "Bad Request",
            json!({ "ok": false, "error": error.to_string() }),
            cors_origin,
        ),
    }
}

async fn submit_browser_capture(
    body: Vec<u8>,
    app_handle: tauri::AppHandle,
    service: CaptureService,
    ai_service: AIEnrichmentService,
) -> AppResult<SubmitCaptureResponse> {
    let payload: BrowserCapturePayload =
        serde_json::from_slice(&body).map_err(|error| AppError::ParseFailed(error.to_string()))?;
    let item = payload.into_raw_capture_item()?;
    let response = service.submit(item).await?;

    let _ = app_handle.emit(
        "capture://browser-submitted",
        json!({
            "objectId": response.object_id,
            "snapshotId": response.snapshot_id,
            "jobId": response.job_id,
            "parsedDocumentId": response.parsed_document_id,
        }),
    );
    let _ = app_handle.emit("library://objects-updated", ());

    if response.parsed_document_id.is_none() {
        spawn_fetch_job_runner(app_handle, service, ai_service, response.job_id.clone());
    } else {
        spawn_ai_enrichment_runner(app_handle, ai_service, response.object_id.clone());
    }

    Ok(response)
}

impl BrowserCapturePayload {
    fn into_raw_capture_item(self) -> AppResult<RawCaptureItem> {
        let url = validate_browser_capture_url(&self.url)?;
        let selected_text = normalize_optional_text(self.selected_text)
            .map(|text| truncate_chars(&text, MAX_RAW_TEXT_CHARS));
        let dom_text = normalize_optional_text(self.dom_text)
            .map(|text| truncate_chars(&text, MAX_RAW_TEXT_CHARS));
        let raw_html = normalize_optional_text(self.dom_html)
            .map(|html| truncate_chars(&html, MAX_RAW_HTML_CHARS));
        let has_selected_text = selected_text.is_some();
        let has_dom_content = dom_text.is_some() || raw_html.is_some();
        let raw_text = selected_text.or(dom_text);
        let source_type = if has_selected_text {
            "selection"
        } else if has_dom_content {
            "dom"
        } else {
            "url"
        };

        Ok(RawCaptureItem {
            id: None,
            user_id: None,
            source_type: source_type.to_string(),
            source_platform: normalize_optional_text(self.source_platform)
                .or_else(|| url.host_str().map(ToOwned::to_owned)),
            source_url: Some(url.as_str().to_string()),
            canonical_url: Some(url.as_str().to_string()),
            title: normalize_optional_text(self.title),
            author: None,
            captured_at: normalize_optional_text(self.captured_at),
            raw_html,
            raw_text,
            assets: Vec::new(),
            metadata: json!({
                "objectType": "article",
                "captureMethod": "browser_extension",
                "captureTransport": "loopback",
            }),
            privacy_level: "personal".to_string(),
            permission_context: PermissionContext {
                acquisition_mode: "user_action".to_string(),
                user_confirmed: true,
                platform_terms_hint: Some("browser extension current page capture".to_string()),
                allowed_for_cloud_processing: false,
                allowed_for_third_party_ai: false,
            },
        })
    }
}

async fn read_http_request(socket: &mut TcpStream) -> AppResult<HttpRequest> {
    let mut buffer = Vec::with_capacity(4096);
    let mut parsed_head: Option<(String, String, HashMap<String, String>, usize, usize)> = None;

    loop {
        let mut chunk = [0_u8; 1024];
        let bytes_read = socket.read(&mut chunk).await?;

        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..bytes_read]);

        if buffer.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return Err(AppError::PolicyDenied(
                "browser capture request exceeds maximum size".to_string(),
            ));
        }

        if parsed_head.is_none() {
            if let Some(header_end) = find_header_end(&buffer) {
                let header_bytes = &buffer[..header_end];
                if header_bytes.len() > MAX_HEADER_BYTES {
                    return Err(AppError::PolicyDenied(
                        "browser capture request headers exceed maximum size".to_string(),
                    ));
                }

                let (method, path, headers, content_length) = parse_http_head(header_bytes)?;
                if content_length > MAX_BODY_BYTES {
                    return Err(AppError::PolicyDenied(
                        "browser capture request body exceeds maximum size".to_string(),
                    ));
                }

                parsed_head = Some((method, path, headers, header_end + 4, content_length));
            }
        }

        if let Some((_, _, _, body_start, content_length)) = &parsed_head {
            if buffer.len() >= body_start + content_length {
                break;
            }
        }
    }

    let Some((method, path, headers, body_start, content_length)) = parsed_head else {
        return Err(AppError::ParseFailed(
            "browser capture request did not contain complete HTTP headers".to_string(),
        ));
    };

    if buffer.len() < body_start + content_length {
        return Err(AppError::ParseFailed(
            "browser capture request body ended before content-length".to_string(),
        ));
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body: buffer[body_start..body_start + content_length].to_vec(),
    })
}

fn parse_http_head(
    header_bytes: &[u8],
) -> AppResult<(String, String, HashMap<String, String>, usize)> {
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|error| AppError::ParseFailed(error.to_string()))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| AppError::ParseFailed("missing HTTP request line".to_string()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| AppError::ParseFailed("missing HTTP method".to_string()))?
        .to_ascii_uppercase();
    let path = request_parts
        .next()
        .ok_or_else(|| AppError::ParseFailed("missing HTTP path".to_string()))?
        .to_string();
    let mut headers = HashMap::new();

    for line in lines {
        if line.is_empty() {
            continue;
        }

        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|error| AppError::ParseFailed(error.to_string()))?
        .unwrap_or(0);

    Ok((method, path, headers, content_length))
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| matches!(window, b"\r\n\r\n"))
}

fn validate_browser_capture_url(raw_url: &str) -> AppResult<Url> {
    let url =
        Url::parse(raw_url.trim()).map_err(|error| AppError::ParseFailed(error.to_string()))?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::PolicyDenied(format!(
            "unsupported browser capture URL scheme: {}",
            url.scheme()
        )));
    }

    Ok(url)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    text.chars().take(max_chars).collect()
}

fn is_json_request(request: &HttpRequest) -> bool {
    request
        .headers
        .get("content-type")
        .is_some_and(|value| value.to_ascii_lowercase().contains("application/json"))
}

fn allowed_cors_origin(request: &HttpRequest) -> Option<String> {
    let origin = request.headers.get("origin")?;

    if origin.starts_with("chrome-extension://")
        || origin.starts_with("moz-extension://")
        || origin == "http://127.0.0.1:1420"
        || origin == "http://localhost:1420"
    {
        return Some(origin.to_string());
    }

    None
}

fn json_response(
    status_code: u16,
    reason: &'static str,
    value: serde_json::Value,
    cors_origin: Option<String>,
) -> HttpResponse {
    let body = serde_json::to_vec(&value).unwrap_or_else(|_| b"{\"ok\":false}".to_vec());
    HttpResponse {
        status_code,
        reason,
        content_type: "application/json; charset=utf-8",
        body,
        cors_origin,
    }
}

fn empty_response(
    status_code: u16,
    reason: &'static str,
    cors_origin: Option<String>,
) -> HttpResponse {
    HttpResponse {
        status_code,
        reason,
        content_type: "text/plain; charset=utf-8",
        body: Vec::new(),
        cors_origin,
    }
}

impl HttpResponse {
    fn into_bytes(self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status_code,
            self.reason,
            self.content_type,
            self.body.len()
        );

        if let Some(origin) = self.cors_origin {
            response.push_str("Access-Control-Allow-Origin: ");
            response.push_str(&origin);
            response.push_str("\r\nVary: Origin\r\n");
            response.push_str("Access-Control-Allow-Headers: content-type\r\n");
            response.push_str("Access-Control-Allow-Methods: POST, OPTIONS\r\n");
            response.push_str("Access-Control-Max-Age: 600\r\n");
        }

        response.push_str("\r\n");
        let mut bytes = response.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::BrowserCapturePayload;

    #[test]
    fn browser_payload_maps_to_confirmed_raw_capture_item() {
        let item = BrowserCapturePayload {
            url: "https://example.com/article".to_string(),
            title: Some("Example Article".to_string()),
            selected_text: Some("Selected useful paragraph".to_string()),
            dom_text: Some("Visible page text".to_string()),
            dom_html: Some("<main><p>Visible page text</p></main>".to_string()),
            source_platform: None,
            captured_at: Some("2026-06-17T00:00:00Z".to_string()),
        }
        .into_raw_capture_item()
        .expect("browser payload should map");

        assert_eq!(item.source_type, "selection");
        assert_eq!(item.source_platform.as_deref(), Some("example.com"));
        assert_eq!(
            item.source_url.as_deref(),
            Some("https://example.com/article")
        );
        assert_eq!(item.title.as_deref(), Some("Example Article"));
        assert_eq!(item.raw_text.as_deref(), Some("Selected useful paragraph"));
        assert!(item.raw_html.is_some());
        assert!(item.permission_context.user_confirmed);
        assert_eq!(item.permission_context.acquisition_mode, "user_action");
        assert!(!item.permission_context.allowed_for_cloud_processing);
        assert!(!item.permission_context.allowed_for_third_party_ai);
    }

    #[test]
    fn browser_payload_rejects_non_http_url() {
        let error = BrowserCapturePayload {
            url: "file:///C:/secret.txt".to_string(),
            title: None,
            selected_text: None,
            dom_text: None,
            dom_html: None,
            source_platform: None,
            captured_at: None,
        }
        .into_raw_capture_item()
        .expect_err("non-http URL should be rejected");

        assert!(error
            .to_string()
            .contains("unsupported browser capture URL scheme"));
    }

    #[test]
    fn browser_payload_without_selection_uses_dom_source_type() {
        let item = BrowserCapturePayload {
            url: "https://example.com/article".to_string(),
            title: Some("Example Article".to_string()),
            selected_text: None,
            dom_text: Some("Visible page text".to_string()),
            dom_html: Some("<main><p>Visible page text</p></main>".to_string()),
            source_platform: None,
            captured_at: None,
        }
        .into_raw_capture_item()
        .expect("browser DOM payload should map");

        assert_eq!(item.source_type, "dom");
        assert_eq!(item.raw_text.as_deref(), Some("Visible page text"));
    }
}
