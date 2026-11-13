//! Xiaohongshu (小红书) adapter.
//!
//! The note page ships the whole note as `window.__INITIAL_STATE__`, including the
//! original image URLs and (for video notes) the stream URL. Reading that payload
//! avoids the client-side renderer and returns the exact title/body the app shows.
//!
//! Login-walled notes still answer with a verification shell; that surfaces as an
//! error so the caller can fall back to the generic pipeline or a browser capture.

use super::{
    browser_user_agent, json_array, json_string, leading_url, markdown_to_text,
    normalize_embedded_json, render_markdown, slice_json_array, slice_json_object, SourceAdapter,
    SourceAsset, SourceAssetKind, SourceCapture, SourceFuture,
};
use crate::errors::{AppError, AppResult};

pub struct XiaohongshuAdapter;

const INITIAL_STATE_MARKER: &str = "window.__INITIAL_STATE__";

impl SourceAdapter for XiaohongshuAdapter {
    fn id(&self) -> &'static str {
        "source.xiaohongshu"
    }

    fn matches(&self, url: &reqwest::Url) -> bool {
        let host = url.host_str().unwrap_or_default();

        // Share short links live on xhslink.com and xhslink.cn.
        if host.starts_with("xhslink.") {
            return true;
        }

        host.ends_with("xiaohongshu.com") && note_id(url).is_some()
    }

    fn capture<'a>(
        &'a self,
        client: &'a reqwest::Client,
        url: reqwest::Url,
        cookies: &'a crate::services::sources::cookies::SourceCookieStore,
    ) -> SourceFuture<'a> {
        Box::pin(async move {
            // App share sheets hand out `xhslink.com` codes, which only carry the note
            // id and the `xsec_token` after the redirect settles.
            let resolved = resolve_share_link(url).await?;
            let note_id = note_id(&resolved)
                .or_else(|| note_id_from_share_code(&resolved))
                .ok_or_else(|| {
                    AppError::ParseFailed("xiaohongshu note url could not be parsed".to_string())
                })?;
            // `/explore/<id>` alone returns a 404 page: the note body is only served
            // when the share link's `xsec_token` comes along.
            let canonical_url = canonical_note_url(&resolved, &note_id);

            let mut request = client
                .get(resolved.as_str())
                .header(reqwest::header::USER_AGENT, browser_user_agent())
                .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
                .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9");

            let stored_cookie = cookies.cookie_for(&resolved).ok_or_else(|| {
                AppError::ParseFailed(format!(
                    "xiaohongshu needs a signed-in cookie for {}: run `sources cookie set {} <cookie>`",
                    resolved.host_str().unwrap_or("xiaohongshu.com"),
                    "xiaohongshu.com"
                ))
            })?;
            request = request.header(reqwest::header::COOKIE, stored_cookie);

            let response = request
                .send()
                .await
                .map_err(|_| AppError::ParseFailed("xiaohongshu request failed".to_string()))?;

            if !response.status().is_success() {
                return Err(AppError::ParseFailed(
                    "xiaohongshu note unavailable".to_string(),
                ));
            }

            // These pages are UTF-8 but may not say so; decode explicitly instead of
            // trusting the response charset, which produced mojibake.
            let bytes = response.bytes().await.map_err(|_| {
                AppError::ParseFailed("xiaohongshu page could not be read".to_string())
            })?;
            let html = String::from_utf8_lossy(&bytes).to_string();

            let notes = extract_note_detail_map(&html).ok_or_else(|| {
                AppError::ParseFailed(
                    "xiaohongshu payload missing: the note requires a signed-in cookie, or the page structure changed".to_string(),
                )
            })?;

            parse_payload(&notes, &note_id, canonical_url).map(Some)
        })
    }
}

/// The whole `__INITIAL_STATE__` is a JavaScript literal (it can contain
/// `new Map([])`), so only the `noteDetailMap` slice is parsed: that part is plain
/// JSON and holds everything the reader needs.
fn extract_note_detail_map(html: &str) -> Option<serde_json::Value> {
    let state_start = html.find(INITIAL_STATE_MARKER)?;
    let state = &html[state_start..];
    let key_index = state.find("\"noteDetailMap\":")?;
    let value = state[key_index + "\"noteDetailMap\":".len()..].trim_start();

    if value.starts_with('{') {
        let slice = slice_json_object(value)?;
        return serde_json::from_str(&normalize_embedded_json(slice)).ok();
    }

    // Older builds serialize the map as `new Map([["id", {...}]])`.
    if let Some(array_start) = value.find('[') {
        let slice = slice_json_array(&value[array_start..])?;
        let entries: serde_json::Value =
            serde_json::from_str(&normalize_embedded_json(slice)).ok()?;
        let mut map = serde_json::Map::new();

        for entry in entries.as_array()? {
            let pair = entry.as_array()?;
            let key = pair.first()?.as_str()?.to_string();
            let value = pair.get(1)?.clone();
            map.insert(key, value);
        }

        return Some(serde_json::Value::Object(map));
    }

    None
}

fn parse_payload(
    payload: &serde_json::Value,
    note_id: &str,
    canonical_url: String,
) -> AppResult<SourceCapture> {
    let note = find_note(payload, note_id)
        .ok_or_else(|| {
            AppError::ParseFailed(
                "xiaohongshu note payload missing: the note requires a signed-in cookie, or the page structure changed"
                    .to_string(),
            )
        })?;

    let title = json_string(note, &["title"]).map(str::to_string);
    let desc = json_string(note, &["desc"]).map(str::to_string);
    let author = json_string(note, &["user", "nickname"]).map(str::to_string);

    let mut assets = Vec::new();

    for image in json_array(note, &["imageList"]) {
        if let Some(url) = leading_url(image, &["urlDefault"])
            .or_else(|| leading_url(image, &["urlPre"]))
            .or_else(|| leading_url(image, &["infoList", "0", "url"]))
            .or_else(|| leading_url(image, &["url"]))
        {
            assets.push(SourceAsset {
                kind: SourceAssetKind::Image,
                url,
            });
        }
    }

    if let Some(url) = leading_url(
        note,
        &["video", "media", "stream", "h264", "0", "masterUrl"],
    )
    .or_else(|| leading_url(note, &["video", "consumer", "originVideoKey"]))
    {
        assets.push(SourceAsset {
            kind: SourceAssetKind::Video,
            url,
        });
    }

    if assets.is_empty() {
        return Err(AppError::ParseFailed(
            "xiaohongshu note has no media".to_string(),
        ));
    }

    let markdown_content = render_markdown(title.as_deref(), desc.as_deref(), &assets);
    let text_content = markdown_to_text(&markdown_content);

    if text_content.trim().is_empty() {
        return Err(AppError::ParseFailed(
            "xiaohongshu note resolved to empty content".to_string(),
        ));
    }

    Ok(SourceCapture {
        parser_id: "source.xiaohongshu",
        canonical_url,
        title: title.or_else(|| author.clone()),
        author,
        text_content,
        markdown_content,
        language: Some("zh-CN".to_string()),
        raw_payload: payload.to_string(),
        assets,
    })
}

fn find_note<'a>(notes: &'a serde_json::Value, note_id: &str) -> Option<&'a serde_json::Value> {
    if let Some(note) = notes
        .get(note_id)
        .and_then(|entry| entry.get("note"))
        .filter(|note| note.is_object())
    {
        return Some(note);
    }

    notes
        .as_object()?
        .values()
        .filter_map(|entry| entry.get("note"))
        .find(|note| note.get("imageList").is_some() || note.get("desc").is_some())
}

async fn resolve_share_link(url: reqwest::Url) -> AppResult<reqwest::Url> {
    let is_share_link = url
        .host_str()
        .map(|host| host.starts_with("xhslink."))
        .unwrap_or(false);

    if !is_share_link {
        return Ok(url);
    }

    // The note URL carries the `xsec_token`, but requesting it anonymously answers
    // with another redirect to `/login`. Read the Location header hop by hop so the
    // token survives instead of following the chain into the login page.
    let resolver = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(browser_user_agent())
        .build()
        .map_err(|_| AppError::ParseFailed("xiaohongshu resolver could not start".to_string()))?;

    let mut current = url;

    for _ in 0..4 {
        let response = resolver
            .get(current.as_str())
            .header(reqwest::header::USER_AGENT, browser_user_agent())
            .send()
            .await
            .map_err(|_| {
                AppError::ParseFailed("xiaohongshu share link request failed".to_string())
            })?;

        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        let next = location
            .and_then(|location| current.join(&location).ok())
            .ok_or_else(|| {
                AppError::ParseFailed(
                    "xiaohongshu share link did not resolve to a note".to_string(),
                )
            })?;

        if next
            .host_str()
            .map(|host| host.ends_with("xiaohongshu.com"))
            .unwrap_or(false)
        {
            return Ok(next);
        }

        current = next;
    }

    Err(AppError::ParseFailed(
        "xiaohongshu share link exceeded the redirect limit".to_string(),
    ))
}

/// Some share redirects land on `/discovery/item/<id>` without a token-bearing
/// path segment; fall back to the last hex segment we can see.
fn note_id_from_share_code(url: &reqwest::Url) -> Option<String> {
    url.path_segments()?
        .rev()
        .find(|segment| is_note_id(segment))
        .map(str::to_string)
}

/// Keep the note id and the share token, drop the tracking params.
fn canonical_note_url(url: &reqwest::Url, note_id: &str) -> String {
    let token = url
        .query_pairs()
        .find(|(key, _)| key == "xsec_token")
        .map(|(_, value)| value.to_string());
    let source = url
        .query_pairs()
        .find(|(key, _)| key == "xsec_source")
        .map(|(_, value)| value.to_string());

    let mut params: Vec<String> = Vec::new();
    if let Some(token) = token {
        params.push(format!("xsec_token={token}"));
    }
    if let Some(source) = source {
        params.push(format!("xsec_source={source}"));
    }

    let mut canonical = format!("https://www.xiaohongshu.com/explore/{note_id}");
    if !params.is_empty() {
        canonical.push('?');
        canonical.push_str(&params.join("&"));
    }

    canonical
}

/// Note id from `/explore/<id>`, `/discovery/item/<id>`, or `?noteId=` style URLs.
fn note_id(url: &reqwest::Url) -> Option<String> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();

    for marker in ["explore", "item", "note"] {
        if let Some(index) = segments.iter().position(|segment| *segment == marker) {
            if let Some(candidate) = segments.get(index + 1) {
                if is_note_id(candidate) {
                    return Some((*candidate).to_string());
                }
            }
        }
    }

    url.query_pairs()
        .find(|(key, _)| key == "noteId" || key == "note_id")
        .map(|(_, value)| value.to_string())
        .filter(|value| is_note_id(value))
}

fn is_note_id(value: &str) -> bool {
    value.len() >= 16
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape taken from a real note page: `noteDetailMap` is keyed by note id.
    const PAYLOAD: &str = r#"{
        "66f0c1d2000000001e02abcd": {
            "note": {
                "noteId": "66f0c1d2000000001e02abcd",
                "title": "一周早餐备餐",
                "desc": "周日晚上做三份，工作日直接热。\n配比写在这里了。",
                "type": "normal",
                "user": { "nickname": "阿栗" },
                "imageList": [
                    { "urlDefault": "https://sns-webpic-qc.xhscdn.com/one.jpg" },
                    { "url": "", "infoList": [{ "url": "https://sns-webpic-qc.xhscdn.com/two.jpg" }] }
                ]
            }
        }
    }"#;

    const VIDEO_PAYLOAD: &str = r#"{
        "x": { "note": {
            "title": "厨房收纳",
            "desc": "视频版",
            "type": "video",
            "imageList": [{ "urlDefault": "https://sns-webpic-qc.xhscdn.com/cover.jpg" }],
            "video": { "media": { "stream": { "h264": [{ "masterUrl": "https://sns-video.xhscdn.com/v.mp4" }] } } }
        } }
    }"#;

    #[test]
    fn matches_note_urls() {
        let adapter = XiaohongshuAdapter;
        let note = reqwest::Url::parse(
            "https://www.xiaohongshu.com/explore/66f0c1d2000000001e02abcd?xsec_token=abc",
        )
        .unwrap();
        let profile = reqwest::Url::parse("https://www.xiaohongshu.com/user/profile/abc").unwrap();

        let share = reqwest::Url::parse("http://xhslink.com/a/AbCdEf123").unwrap();
        let share_cn = reqwest::Url::parse("https://xhslink.cn/o/1BJ9SUBuuP1").unwrap();

        assert!(adapter.matches(&note));
        assert!(adapter.matches(&share), "app share short links must match");
        assert!(adapter.matches(&share_cn), "xhslink.cn must match");
        assert!(!adapter.matches(&profile));

        let canonical = canonical_note_url(&note, "66f0c1d2000000001e02abcd");
        assert!(canonical.contains("xsec_token=abc"));
        assert!(!canonical.contains("xhsshare"));
    }

    #[test]
    fn parses_note_text_and_original_images() {
        let payload: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        let capture = parse_payload(
            &payload,
            "66f0c1d2000000001e02abcd",
            "https://www.xiaohongshu.com/explore/66f0c1d2000000001e02abcd".to_string(),
        )
        .expect("capture");

        assert_eq!(capture.title.as_deref(), Some("一周早餐备餐"));
        assert_eq!(capture.author.as_deref(), Some("阿栗"));
        assert_eq!(capture.assets.len(), 2);
        assert!(capture
            .markdown_content
            .contains("![image](https://sns-webpic-qc.xhscdn.com/one.jpg)"));
        assert!(capture.text_content.contains("周日晚上做三份"));
    }

    #[test]
    fn appends_video_asset_for_video_notes() {
        let payload: serde_json::Value = serde_json::from_str(VIDEO_PAYLOAD).unwrap();
        let capture = parse_payload(
            &payload,
            "x",
            "https://www.xiaohongshu.com/explore/x".to_string(),
        )
        .expect("capture");

        assert_eq!(capture.assets.len(), 2);
        assert!(capture
            .markdown_content
            .contains("[video](https://sns-video.xhscdn.com/v.mp4)"));
    }

    #[test]
    fn rejects_payload_without_media() {
        let payload = serde_json::json!({ "x": { "note": { "title": "t", "desc": "d" } } });

        assert!(parse_payload(&payload, "x", "https://x".to_string()).is_err());
    }
}
