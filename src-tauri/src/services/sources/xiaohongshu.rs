//! Xiaohongshu (小红书) adapter.
//!
//! The note page ships the whole note as `window.__INITIAL_STATE__`, including the
//! original image URLs and (for video notes) the stream URL. Reading that payload
//! avoids the client-side renderer and returns the exact title/body the app shows.
//!
//! Login-walled notes still answer with a verification shell; that surfaces as an
//! error so the caller can fall back to the generic pipeline or a browser capture.

use super::{
    browser_user_agent, extract_embedded_json, json_array, json_string, leading_url,
    markdown_to_text, render_markdown, SourceAdapter, SourceAsset, SourceAssetKind, SourceCapture,
    SourceFuture,
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
        host.ends_with("xiaohongshu.com") && note_id(url).is_some()
    }

    fn capture<'a>(&'a self, client: &'a reqwest::Client, url: reqwest::Url) -> SourceFuture<'a> {
        Box::pin(async move {
            let note_id = note_id(&url).ok_or_else(|| {
                AppError::ParseFailed("xiaohongshu note url could not be parsed".to_string())
            })?;
            let canonical_url = format!("https://www.xiaohongshu.com/explore/{note_id}");

            let response = client
                .get(&canonical_url)
                .header(reqwest::header::USER_AGENT, browser_user_agent())
                .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
                .send()
                .await
                .map_err(|_| AppError::ParseFailed("xiaohongshu request failed".to_string()))?;

            if !response.status().is_success() {
                return Err(AppError::ParseFailed(
                    "xiaohongshu note unavailable".to_string(),
                ));
            }

            let html = response
                .text()
                .await
                .map_err(|_| AppError::ParseFailed("xiaohongshu page was not text".to_string()))?;

            let payload = extract_embedded_json(&html, INITIAL_STATE_MARKER).ok_or_else(|| {
                AppError::ParseFailed(
                    "xiaohongshu payload missing (login wall or structure change)".to_string(),
                )
            })?;

            parse_payload(&payload, &note_id, canonical_url).map(Some)
        })
    }
}

fn parse_payload(
    payload: &serde_json::Value,
    note_id: &str,
    canonical_url: String,
) -> AppResult<SourceCapture> {
    let note = find_note(payload, note_id)
        .ok_or_else(|| AppError::ParseFailed("xiaohongshu note payload missing".to_string()))?;

    let title = json_string(note, &["title"]).map(str::to_string);
    let desc = json_string(note, &["desc"]).map(str::to_string);
    let author = json_string(note, &["user", "nickname"]).map(str::to_string);

    let mut assets = Vec::new();

    for image in json_array(note, &["imageList"]) {
        if let Some(url) = leading_url(image, &["urlDefault"])
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

fn find_note<'a>(payload: &'a serde_json::Value, note_id: &str) -> Option<&'a serde_json::Value> {
    let detail_map = payload.get("note")?.get("noteDetailMap")?;

    if let Some(note) = detail_map
        .get(note_id)
        .and_then(|entry| entry.get("note"))
        .filter(|note| note.is_object())
    {
        return Some(note);
    }

    detail_map
        .as_object()?
        .values()
        .filter_map(|entry| entry.get("note"))
        .find(|note| note.get("imageList").is_some() || note.get("desc").is_some())
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

    const PAYLOAD: &str = r#"{
        "note": {
            "noteDetailMap": {
                "66f0c1d2000000001e02abcd": {
                    "note": {
                        "noteId": "66f0c1d2000000001e02abcd",
                        "title": "一周早餐备餐",
                        "desc": "周日晚上做三份，工作日直接热。\n配比写在这里了。",
                        "type": "normal",
                        "user": { "nickname": "阿栗" },
                        "imageList": [
                            { "urlDefault": "https://sns-webpic-qc.xhscdn.com/one.jpg" },
                            { "infoList": [{ "url": "https://sns-webpic-qc.xhscdn.com/two.jpg" }] }
                        ]
                    }
                }
            }
        }
    }"#;

    const VIDEO_PAYLOAD: &str = r#"{
        "note": { "noteDetailMap": { "x": { "note": {
            "title": "厨房收纳",
            "desc": "视频版",
            "type": "video",
            "imageList": [{ "urlDefault": "https://sns-webpic-qc.xhscdn.com/cover.jpg" }],
            "video": { "media": { "stream": { "h264": [{ "masterUrl": "https://sns-video.xhscdn.com/v.mp4" }] } } }
        } } } }
    }"#;

    #[test]
    fn matches_note_urls() {
        let adapter = XiaohongshuAdapter;
        let note = reqwest::Url::parse(
            "https://www.xiaohongshu.com/explore/66f0c1d2000000001e02abcd?xsec_token=abc",
        )
        .unwrap();
        let profile = reqwest::Url::parse("https://www.xiaohongshu.com/user/profile/abc").unwrap();

        assert!(adapter.matches(&note));
        assert!(!adapter.matches(&profile));
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
        let payload = serde_json::json!({
            "note": { "noteDetailMap": { "x": { "note": { "title": "t", "desc": "d" } } } }
        });

        assert!(parse_payload(&payload, "x", "https://x".to_string()).is_err());
    }
}
