//! Platform-specific source adapters.
//!
//! Some platforms never ship their content in the rendered DOM for an anonymous
//! fetch: they embed it as JSON in the page (`__INITIAL_STATE__`, `_ROUTER_DATA`)
//! or expose a public JSON endpoint. Fetching that payload gives the exact
//! original text and media URLs without scraping or OCR.
//!
//! Each adapter is responsible for one host and returns a `SourceCapture` when it
//! could resolve the payload. Returning `None` means "not my URL"; an `Err` means
//! "this is my URL but the payload could not be read" (structure changed, blocked,
//! removed) so the caller can fall back to the generic HTML pipeline.

use crate::errors::AppResult;
use std::future::Future;
use std::pin::Pin;

pub mod cookies;
pub mod douyin;
pub mod x;
pub mod xiaohongshu;
pub mod zhihu;

/// Media referenced by a captured source, kept in original quality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAsset {
    pub kind: SourceAssetKind,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAssetKind {
    Image,
    Video,
}

impl SourceAssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

/// Normalized result of a platform adapter, ready for the capture pipeline.
#[derive(Debug, Clone)]
pub struct SourceCapture {
    /// Stable parser id recorded on the snapshot for audit.
    pub parser_id: &'static str,
    /// Canonical URL the content came from.
    pub canonical_url: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub text_content: String,
    pub markdown_content: String,
    pub language: Option<String>,
    /// Raw payload (JSON/HTML) stored as the source snapshot.
    pub raw_payload: String,
    pub assets: Vec<SourceAsset>,
}

pub type SourceFuture<'a> =
    Pin<Box<dyn Future<Output = AppResult<Option<SourceCapture>>> + Send + 'a>>;

pub trait SourceAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches(&self, url: &reqwest::Url) -> bool;
    fn capture<'a>(
        &'a self,
        client: &'a reqwest::Client,
        url: reqwest::Url,
        cookies: &'a cookies::SourceCookieStore,
    ) -> SourceFuture<'a>;
}

/// Platform metadata surfaced in Settings → Sources.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePlatform {
    pub id: &'static str,
    pub label: &'static str,
    /// Domain the cookie is stored under; `None` means the adapter needs none.
    pub cookie_domain: Option<&'static str>,
}

pub fn platforms() -> Vec<SourcePlatform> {
    vec![
        SourcePlatform {
            id: x::XAdapter.id(),
            label: "X / Twitter",
            cookie_domain: None,
        },
        SourcePlatform {
            id: xiaohongshu::XiaohongshuAdapter.id(),
            label: "小红书",
            cookie_domain: Some("xiaohongshu.com"),
        },
        SourcePlatform {
            id: douyin::DouyinAdapter.id(),
            label: "抖音",
            cookie_domain: Some("douyin.com"),
        },
        SourcePlatform {
            id: zhihu::ZhihuAdapter.id(),
            label: "知乎",
            cookie_domain: Some("zhihu.com"),
        },
    ]
}

/// All adapters in resolution order.
pub fn adapters() -> Vec<Box<dyn SourceAdapter>> {
    vec![
        Box::new(x::XAdapter),
        Box::new(xiaohongshu::XiaohongshuAdapter),
        Box::new(douyin::DouyinAdapter),
        Box::new(zhihu::ZhihuAdapter),
    ]
}

/// Resolve a URL through the first matching adapter.
///
/// `Ok(None)` means no adapter claims the URL; `Ok(Some(capture))` is a resolved
/// source; `Err` must be treated as "adapter failed" by the caller.
pub async fn capture_with_adapter(
    client: &reqwest::Client,
    url: &reqwest::Url,
    cookies: &cookies::SourceCookieStore,
) -> AppResult<Option<SourceCapture>> {
    for adapter in adapters() {
        if !adapter.matches(url) {
            continue;
        }

        return adapter.capture(client, url.clone(), cookies).await;
    }

    Ok(None)
}

pub(crate) fn browser_user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
}

pub(crate) fn mobile_user_agent() -> &'static str {
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1"
}

/// Extract the JSON object assigned to `marker` inside an inline script.
///
/// Platform payloads are not strict JSON: they contain bare `undefined` and
/// escaped forward slashes. Both are normalized before parsing.
pub(crate) fn extract_embedded_json(html: &str, marker: &str) -> Option<serde_json::Value> {
    let start = html.find(marker)? + marker.len();
    let rest = html[start..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    let end = json_object_end(rest)?;
    let payload = normalize_embedded_json(&rest[..end]);

    serde_json::from_str(&payload).ok()
}

/// Platform payloads are JavaScript literals: `undefined` may appear as a value.
/// Only rewrite the bare token outside strings, and keep the surrounding
/// punctuation intact (`"key":undefined` must become `"key":null`).
pub(crate) fn normalize_embedded_json(payload: &str) -> String {
    let bytes = payload.as_bytes();
    let mut out = String::with_capacity(payload.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;

    while index < bytes.len() {
        // `index` only ever advances on char boundaries, so slicing is safe.
        let rest = &payload[index..];
        let character = rest.chars().next().expect("index is inside the string");

        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            out.push(character);
            index += character.len_utf8();
            continue;
        }

        if character == '"' {
            in_string = true;
            out.push(character);
            index += 1;
            continue;
        }

        if rest.starts_with("undefined") {
            let before_ok = index == 0 || !is_identifier_byte(bytes[index - 1]);
            let after = index + "undefined".len();
            let after_ok = after >= bytes.len() || !is_identifier_byte(bytes[after]);

            if before_ok && after_ok {
                out.push_str("null");
                index = after;
                continue;
            }
        }

        out.push(character);
        index += character.len_utf8();
    }

    out.replace("\\u002F", "/")
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Slice of a JSON object that starts at `text[0]`, or `None` when unbalanced.
pub(crate) fn slice_json_object(text: &str) -> Option<&str> {
    json_object_end(text).map(|end| &text[..end])
}

/// Slice of a JSON array that starts at `text[0]`, or `None` when unbalanced.
pub(crate) fn slice_json_array(text: &str) -> Option<&str> {
    json_array_end(text).map(|end| &text[..end])
}

/// Find the end offset of the JSON array starting at `text[0]`, honoring strings.
fn json_array_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }

    None
}

/// Find the end offset of the JSON object starting at `text[0]`, honoring strings.
fn json_object_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }

    None
}

/// Build markdown from a title/body and the original media, preserving order.
pub(crate) fn render_markdown(
    title: Option<&str>,
    body: Option<&str>,
    assets: &[SourceAsset],
) -> String {
    let mut markdown = String::new();

    if let Some(title) = title.map(str::trim).filter(|value| !value.is_empty()) {
        markdown.push_str("# ");
        markdown.push_str(title);
        markdown.push_str("\n\n");
    }

    if let Some(body) = body.map(str::trim).filter(|value| !value.is_empty()) {
        markdown.push_str(body);
        markdown.push_str("\n\n");
    }

    for asset in assets {
        match asset.kind {
            SourceAssetKind::Image => {
                markdown.push_str("![image](");
                markdown.push_str(&asset.url);
                markdown.push_str(")\n");
            }
            SourceAssetKind::Video => {
                markdown.push_str("[video](");
                markdown.push_str(&asset.url);
                markdown.push_str(")\n");
            }
        }
    }

    markdown.trim_end().to_string()
}

/// Plain-text projection of a markdown document for search and snapshot hashing.
pub(crate) fn markdown_to_text(markdown: &str) -> String {
    let mut text = String::with_capacity(markdown.len());

    for line in markdown.lines() {
        let trimmed = line.trim_start_matches('#').trim();
        if trimmed.is_empty() || trimmed.starts_with("![") || trimmed.starts_with("[video](") {
            continue;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(trimmed);
    }

    text.trim().to_string()
}

/// Walk a JSON path; numeric segments index into arrays.
pub(crate) fn get_path<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = value;

    for key in path {
        current = match current {
            serde_json::Value::Array(items) => items.get(key.parse::<usize>().ok()?)?,
            _ => current.get(*key)?,
        };
    }

    Some(current)
}

fn json_string<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    get_path(value, path)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
}

fn json_array<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a [serde_json::Value] {
    get_path(value, path)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn leading_url(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let current = get_path(value, path)?;

    if let Some(url) = current.as_str() {
        return Some(url.to_string()).filter(|url| !url.trim().is_empty());
    }

    current
        .as_array()?
        .iter()
        .filter_map(|entry| entry.as_str())
        .find(|url| !url.trim().is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_embedded_json_with_undefined_and_escaped_slashes() {
        let html = r#"<script>window.__INITIAL_STATE__={"a":undefined,"b":"https:\/\/example.com\/x.png"}</script>"#;

        let value = extract_embedded_json(html, "window.__INITIAL_STATE__").expect("payload");

        assert!(value["a"].is_null());
        assert_eq!(value["b"], "https://example.com/x.png");
    }

    #[test]
    fn keeps_multibyte_text_intact() {
        let html = r#"<script>window.__INITIAL_STATE__={"title":"人，刷到请打开你的 GPT。","x":undefined}</script>"#;

        let value = extract_embedded_json(html, "window.__INITIAL_STATE__").expect("payload");

        assert_eq!(value["title"], "人，刷到请打开你的 GPT。");
        assert!(value["x"].is_null());
    }

    #[test]
    fn keeps_punctuation_when_replacing_bare_undefined() {
        let html = r#"<script>window.__INITIAL_STATE__={"a":undefined,"b":1,"c":"undefined text"}</script>"#;

        let value = extract_embedded_json(html, "window.__INITIAL_STATE__").expect("payload");

        assert!(value["a"].is_null());
        assert_eq!(value["b"], 1);
        assert_eq!(value["c"], "undefined text");
    }

    #[test]
    fn ignores_braces_inside_strings_when_finding_the_end() {
        let html = r#"<script>window._ROUTER_DATA = {"text":"a } brace"}</script>"#;

        let value = extract_embedded_json(html, "window._ROUTER_DATA").expect("payload");

        assert_eq!(value["text"], "a } brace");
    }

    #[test]
    fn renders_title_body_and_media_in_order() {
        let assets = vec![
            SourceAsset {
                kind: SourceAssetKind::Image,
                url: "https://cdn.example.com/1.jpg".to_string(),
            },
            SourceAsset {
                kind: SourceAssetKind::Video,
                url: "https://cdn.example.com/v.mp4".to_string(),
            },
        ];

        let markdown = render_markdown(Some("Title"), Some("Body"), &assets);

        assert_eq!(
            markdown,
            "# Title\n\nBody\n\n![image](https://cdn.example.com/1.jpg)\n[video](https://cdn.example.com/v.mp4)"
        );
    }

    #[test]
    fn markdown_to_text_drops_media_lines() {
        let text = markdown_to_text("# Title\n\nBody line\n\n![image](https://x/1.jpg)");

        assert_eq!(text, "Title\nBody line");
    }

    #[test]
    fn json_helpers_read_nested_paths() {
        let value = serde_json::json!({
            "a": { "b": [{ "url": "https://x/1.jpg" }] },
            "list": [{ "url_list": ["https://x/2.jpg"] }]
        });

        assert_eq!(
            json_string(&value, &["a", "b", "0", "url"]),
            Some("https://x/1.jpg")
        );
        assert_eq!(json_string(&value, &["a", "b", "1", "url"]), None);
        assert_eq!(
            leading_url(&value["list"][0], &["url_list"]),
            Some("https://x/2.jpg".to_string())
        );
        assert_eq!(json_array(&value, &["list"]).len(), 1);
    }
}
