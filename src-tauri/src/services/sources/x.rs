//! X / Twitter adapter.
//!
//! X renders tweets on the client, so an anonymous HTML fetch yields a shell.
//! Two public JSON endpoints expose the same tweet payload the app renders:
//! `api.fxtwitter.com` (preferred, richer media) and the syndication endpoint
//! used by embedded tweets (fallback). Both work without credentials.

use super::{
    browser_user_agent, json_string, leading_url, markdown_to_text, render_markdown, SourceAdapter,
    SourceAsset, SourceAssetKind, SourceCapture, SourceFuture,
};
use crate::errors::{AppError, AppResult};

pub struct XAdapter;

const FX_TWITTER_API: &str = "https://api.fxtwitter.com";
const SYNDICATION_API: &str = "https://cdn.syndication.twimg.com/tweet-result";

impl SourceAdapter for XAdapter {
    fn id(&self) -> &'static str {
        "source.x"
    }

    fn matches(&self, url: &reqwest::Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        matches!(
            host,
            "x.com" | "www.x.com" | "twitter.com" | "www.twitter.com" | "mobile.twitter.com"
        ) && tweet_path(url).is_some()
    }

    fn capture<'a>(
        &'a self,
        client: &'a reqwest::Client,
        url: reqwest::Url,
        _cookie: Option<&'a str>,
    ) -> SourceFuture<'a> {
        Box::pin(async move {
            let (user, tweet_id) = tweet_path(&url).ok_or_else(|| {
                AppError::ParseFailed("x status url could not be parsed".to_string())
            })?;

            let canonical_url = format!("https://x.com/{user}/status/{tweet_id}");

            let payload = match fetch_fx_twitter(client, &canonical_url).await {
                Ok(payload) => payload,
                Err(_) => fetch_syndication(client, &tweet_id).await?,
            };

            parse_payload(&payload, canonical_url).map(Some)
        })
    }
}

async fn fetch_fx_twitter(
    client: &reqwest::Client,
    canonical_url: &str,
) -> AppResult<serde_json::Value> {
    let api_url = canonical_url.replacen("https://x.com/", &format!("{FX_TWITTER_API}/"), 1);

    let response = client
        .get(api_url)
        .header(reqwest::header::USER_AGENT, browser_user_agent())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| AppError::ParseFailed("x source request failed".to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::ParseFailed("x source unavailable".to_string()));
    }

    response
        .json()
        .await
        .map_err(|_| AppError::ParseFailed("x source payload was not json".to_string()))
}

async fn fetch_syndication(
    client: &reqwest::Client,
    tweet_id: &str,
) -> AppResult<serde_json::Value> {
    let url = format!("{SYNDICATION_API}?id={tweet_id}&token=a");
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, browser_user_agent())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| AppError::ParseFailed("x syndication request failed".to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::ParseFailed(
            "x syndication endpoint unavailable".to_string(),
        ));
    }

    response
        .json()
        .await
        .map_err(|_| AppError::ParseFailed("x syndication payload was not json".to_string()))
}

/// Accepts either the fxtwitter or the syndication payload shape.
fn parse_payload(payload: &serde_json::Value, canonical_url: String) -> AppResult<SourceCapture> {
    let tweet = payload.get("tweet").unwrap_or(payload);

    let text = json_string(tweet, &["text"])
        .or_else(|| json_string(tweet, &["full_text"]))
        .ok_or_else(|| AppError::ParseFailed("x tweet has no text".to_string()))?;

    let author = json_string(tweet, &["author", "name"])
        .or_else(|| json_string(tweet, &["user", "name"]))
        .map(str::to_string)
        .or_else(|| {
            json_string(tweet, &["author", "screen_name"])
                .or_else(|| json_string(tweet, &["user", "screen_name"]))
                .map(|handle| format!("@{handle}"))
        });

    let assets = collect_assets(tweet);
    let markdown_content = render_markdown(None, Some(text), &assets);
    let text_content = markdown_to_text(&markdown_content);

    if text_content.trim().is_empty() {
        return Err(AppError::ParseFailed(
            "x tweet resolved to empty content".to_string(),
        ));
    }

    Ok(SourceCapture {
        parser_id: "source.x",
        canonical_url,
        title: author.clone(),
        author,
        text_content,
        markdown_content,
        language: Some("en".to_string()),
        raw_payload: payload.to_string(),
        assets,
    })
}

fn collect_assets(tweet: &serde_json::Value) -> Vec<SourceAsset> {
    let mut assets = Vec::new();

    for photo in tweet
        .get("media")
        .and_then(|media| media.get("photos"))
        .and_then(|photos| photos.as_array())
        .into_iter()
        .flatten()
        .chain(
            tweet
                .get("photos")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten(),
        )
    {
        if let Some(url) = photo.get("url").and_then(|value| value.as_str()) {
            assets.push(SourceAsset {
                kind: SourceAssetKind::Image,
                url: url.to_string(),
            });
        }
    }

    for video in tweet
        .get("media")
        .and_then(|media| media.get("videos"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        if let Some(url) = video.get("url").and_then(|value| value.as_str()) {
            assets.push(SourceAsset {
                kind: SourceAssetKind::Video,
                url: url.to_string(),
            });
            continue;
        }

        if let Some(url) = leading_url(video, &["variants", "0", "src"]) {
            assets.push(SourceAsset {
                kind: SourceAssetKind::Video,
                url,
            });
        }
    }

    if let Some(url) = leading_url(tweet, &["video", "variants", "0", "src"]) {
        assets.push(SourceAsset {
            kind: SourceAssetKind::Video,
            url,
        });
    }

    assets
}

/// Extract `(user, tweet_id)` from `/user/status/<id>` style paths.
fn tweet_path(url: &reqwest::Url) -> Option<(String, String)> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();

    let status_index = segments.iter().position(|segment| *segment == "status")?;
    let tweet_id = segments.get(status_index + 1)?;
    if !tweet_id.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }

    let user = if status_index > 0 {
        segments[0].to_string()
    } else {
        "i".to_string()
    };

    Some((user, (*tweet_id).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FX_PAYLOAD: &str = r#"{
        "code": 200,
        "tweet": {
            "text": "Shipping a local-first tool today. https://t.co/abc",
            "author": { "name": "Jane Doe", "screen_name": "janedoe" },
            "media": {
                "photos": [
                    { "type": "photo", "url": "https://pbs.twimg.com/media/one.jpg" },
                    { "type": "photo", "url": "https://pbs.twimg.com/media/two.jpg" }
                ],
                "videos": [
                    { "type": "video", "url": "https://video.twimg.com/clip.mp4" }
                ]
            }
        }
    }"#;

    const SYNDICATION_PAYLOAD: &str = r#"{
        "text": "A thread about SQLite",
        "user": { "name": "Sam", "screen_name": "sam" },
        "photos": [{ "url": "https://pbs.twimg.com/media/three.jpg" }],
        "video": { "variants": [{ "src": "https://video.twimg.com/thread.mp4" }] }
    }"#;

    #[test]
    fn matches_status_urls_only() {
        let adapter = XAdapter;
        let status =
            reqwest::Url::parse("https://x.com/janedoe/status/1812345678901234567").unwrap();
        let profile = reqwest::Url::parse("https://x.com/janedoe").unwrap();
        let other = reqwest::Url::parse("https://example.com/janedoe/status/1").unwrap();

        assert!(adapter.matches(&status));
        assert!(!adapter.matches(&profile));
        assert!(!adapter.matches(&other));
    }

    #[test]
    fn parses_fx_payload_with_original_media() {
        let payload: serde_json::Value = serde_json::from_str(FX_PAYLOAD).unwrap();
        let capture = parse_payload(
            &payload,
            "https://x.com/janedoe/status/1812345678901234567".to_string(),
        )
        .expect("capture");

        assert_eq!(capture.author.as_deref(), Some("Jane Doe"));
        assert!(capture.text_content.contains("local-first tool"));
        assert_eq!(capture.assets.len(), 3);
        assert!(capture
            .markdown_content
            .contains("![image](https://pbs.twimg.com/media/one.jpg)"));
        assert!(capture
            .markdown_content
            .contains("[video](https://video.twimg.com/clip.mp4)"));
    }

    #[test]
    fn parses_syndication_payload_as_fallback() {
        let payload: serde_json::Value = serde_json::from_str(SYNDICATION_PAYLOAD).unwrap();
        let capture =
            parse_payload(&payload, "https://x.com/sam/status/1".to_string()).expect("capture");

        assert_eq!(capture.author.as_deref(), Some("Sam"));
        assert!(capture.text_content.contains("A thread about SQLite"));
        assert_eq!(capture.assets.len(), 2);
    }

    #[test]
    fn rejects_payload_without_text() {
        let payload = serde_json::json!({ "tweet": { "author": { "name": "x" } } });

        assert!(parse_payload(&payload, "https://x.com/a/status/1".to_string()).is_err());
    }
}
