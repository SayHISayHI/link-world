//! Douyin (抖音) adapter.
//!
//! Share links (`v.douyin.com/xxx`) redirect to an `iesdouyin.com/share/...` page
//! that embeds the full work description plus original image / video URLs in
//! `window._ROUTER_DATA`. Public works need no login.

use super::{
    extract_embedded_json, json_array, json_string, leading_url, markdown_to_text,
    mobile_user_agent, render_markdown, SourceAdapter, SourceAsset, SourceAssetKind, SourceCapture,
    SourceFuture,
};
use crate::errors::{AppError, AppResult};

pub struct DouyinAdapter;

const ROUTER_DATA_MARKER: &str = "window._ROUTER_DATA";

impl SourceAdapter for DouyinAdapter {
    fn id(&self) -> &'static str {
        "source.douyin"
    }

    fn matches(&self, url: &reqwest::Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        host.ends_with("douyin.com")
    }

    fn capture<'a>(
        &'a self,
        client: &'a reqwest::Client,
        url: reqwest::Url,
        _cookies: &'a crate::services::sources::cookies::SourceCookieStore,
    ) -> SourceFuture<'a> {
        Box::pin(async move {
            // Share links redirect to the canonical work URL; follow it first.
            let resolved = resolve_work_url(client, url).await?;
            let work_id = work_id(&resolved).ok_or_else(|| {
                AppError::ParseFailed("douyin work url could not be parsed".to_string())
            })?;

            let share_url = format!("https://www.iesdouyin.com/share/video/{work_id}/");
            let response = client
                .get(&share_url)
                .header(reqwest::header::USER_AGENT, mobile_user_agent())
                .send()
                .await
                .map_err(|_| AppError::ParseFailed("douyin request failed".to_string()))?;

            if !response.status().is_success() {
                return Err(AppError::ParseFailed("douyin work unavailable".to_string()));
            }

            let html = response
                .text()
                .await
                .map_err(|_| AppError::ParseFailed("douyin page was not text".to_string()))?;

            let payload = extract_embedded_json(&html, ROUTER_DATA_MARKER).ok_or_else(|| {
                AppError::ParseFailed("douyin payload missing (structure change)".to_string())
            })?;

            parse_payload(&payload, format!("https://www.douyin.com/video/{work_id}")).map(Some)
        })
    }
}

async fn resolve_work_url(
    client: &reqwest::Client,
    mut url: reqwest::Url,
) -> AppResult<reqwest::Url> {
    if work_id(&url).is_some() {
        return Ok(url);
    }

    let response = client
        .get(url.as_str())
        .header(reqwest::header::USER_AGENT, mobile_user_agent())
        .send()
        .await
        .map_err(|_| AppError::ParseFailed("douyin share link request failed".to_string()))?;

    url = response.url().clone();
    let _ = response.bytes().await;

    if work_id(&url).is_none() {
        return Err(AppError::ParseFailed(
            "douyin share link did not resolve to a work".to_string(),
        ));
    }

    Ok(url)
}

fn parse_payload(payload: &serde_json::Value, canonical_url: String) -> AppResult<SourceCapture> {
    let item = find_item(payload)
        .ok_or_else(|| AppError::ParseFailed("douyin work payload missing".to_string()))?;

    let desc = json_string(item, &["desc"]).map(str::to_string);
    let author = json_string(item, &["author", "nickname"]).map(str::to_string);

    let mut assets = Vec::new();

    for image in json_array(item, &["images"]) {
        if let Some(url) = leading_url(image, &["url_list"]) {
            assets.push(SourceAsset {
                kind: SourceAssetKind::Image,
                url,
            });
        }
    }

    if let Some(url) = leading_url(item, &["video", "play_addr", "url_list"])
        .or_else(|| leading_url(item, &["video", "playAddr", "urlList"]))
    {
        assets.push(SourceAsset {
            kind: SourceAssetKind::Video,
            url,
        });
    }

    if assets.is_empty() {
        return Err(AppError::ParseFailed(
            "douyin work has no media".to_string(),
        ));
    }

    let markdown_content = render_markdown(None, desc.as_deref(), &assets);
    let text_content = markdown_to_text(&markdown_content);

    if text_content.trim().is_empty() {
        return Err(AppError::ParseFailed(
            "douyin work resolved to empty content".to_string(),
        ));
    }

    Ok(SourceCapture {
        parser_id: "source.douyin",
        canonical_url,
        title: desc.clone().filter(|value| !value.trim().is_empty()),
        author,
        text_content,
        markdown_content,
        language: Some("zh-CN".to_string()),
        raw_payload: payload.to_string(),
        assets,
    })
}

/// `_ROUTER_DATA.loaderData` is keyed by route, e.g. `video_(id)/page`.
fn find_item(payload: &serde_json::Value) -> Option<&serde_json::Value> {
    let loader_data = payload.get("loaderData")?;

    loader_data
        .as_object()?
        .values()
        .filter_map(|page| page.get("videoInfoRes"))
        .filter_map(|res| res.get("item_list").or_else(|| res.get("itemList")))
        .filter_map(|list| list.as_array())
        .filter_map(|list| list.first())
        .next()
}

fn work_id(url: &reqwest::Url) -> Option<String> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();

    for marker in ["video", "note", "slides"] {
        if let Some(index) = segments.iter().position(|segment| *segment == marker) {
            if let Some(candidate) = segments.get(index + 1) {
                if is_work_id(candidate) {
                    return Some((*candidate).to_string());
                }
            }
        }
    }

    None
}

fn is_work_id(value: &str) -> bool {
    value.len() >= 12 && value.chars().all(|character| character.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE_PAYLOAD: &str = r#"{
        "loaderData": {
            "video_(id)/page": {
                "videoInfoRes": {
                    "item_list": [{
                        "desc": "三种收纳盒怎么选\n看完不踩坑",
                        "author": { "nickname": "收纳小张" },
                        "images": [
                            { "url_list": ["https://p3-pc-sign.douyinpic.com/one.jpeg"] },
                            { "url_list": ["https://p3-pc-sign.douyinpic.com/two.jpeg"] }
                        ]
                    }]
                }
            }
        }
    }"#;

    const VIDEO_PAYLOAD: &str = r#"{
        "loaderData": {
            "video_(id)/page": {
                "videoInfoRes": {
                    "item_list": [{
                        "desc": "三分钟看完这本书",
                        "author": { "nickname": "读书号" },
                        "video": { "play_addr": { "url_list": ["https://v3.douyinvod.com/v.mp4"] } }
                    }]
                }
            }
        }
    }"#;

    #[test]
    fn matches_douyin_hosts() {
        let adapter = DouyinAdapter;
        let short = reqwest::Url::parse("https://v.douyin.com/abc123/").unwrap();
        let canonical =
            reqwest::Url::parse("https://www.douyin.com/video/7350000000000000000").unwrap();

        assert!(adapter.matches(&short));
        assert!(adapter.matches(&canonical));
    }

    #[test]
    fn parses_image_post_with_original_images() {
        let payload: serde_json::Value = serde_json::from_str(IMAGE_PAYLOAD).unwrap();
        let capture = parse_payload(
            &payload,
            "https://www.douyin.com/video/7350000000000000000".to_string(),
        )
        .expect("capture");

        assert_eq!(capture.author.as_deref(), Some("收纳小张"));
        assert_eq!(capture.assets.len(), 2);
        assert!(capture.text_content.contains("三种收纳盒怎么选"));
        assert!(capture
            .markdown_content
            .contains("![image](https://p3-pc-sign.douyinpic.com/one.jpeg)"));
    }

    #[test]
    fn parses_video_post_with_play_url() {
        let payload: serde_json::Value = serde_json::from_str(VIDEO_PAYLOAD).unwrap();
        let capture =
            parse_payload(&payload, "https://www.douyin.com/video/1".to_string()).expect("capture");

        assert!(capture
            .markdown_content
            .contains("[video](https://v3.douyinvod.com/v.mp4)"));
    }

    #[test]
    fn extracts_work_id_from_share_and_canonical_urls() {
        assert_eq!(
            work_id(
                &reqwest::Url::parse("https://www.douyin.com/video/7350000000000000000").unwrap()
            )
            .as_deref(),
            Some("7350000000000000000")
        );
        assert_eq!(
            work_id(&reqwest::Url::parse("https://v.douyin.com/abc123/").unwrap()),
            None
        );
    }
}
