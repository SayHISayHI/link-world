//! Zhihu (知乎) adapter.
//!
//! Zhihu serves the answer body twice: rendered in `div.RichText` for bots and in
//! the `js-initialData` JSON blob for the client. The JSON is the reliable source
//! (it survives layout changes and includes the author and question title), so it
//! is preferred, with the rendered fragment as a fallback.
//!
//! A signed-in cookie is required: anonymous requests receive a 403 anti-bot page.

use super::{
    browser_user_agent, map_source_request_error, markdown_to_text, render_markdown, SourceAdapter,
    SourceAsset, SourceAssetKind, SourceCapture, SourceFuture,
};
use crate::errors::{AppError, AppResult};
use crate::services::document_parser::{parse_html_document, DocumentHints};
use scraper::{Html, Selector};

pub struct ZhihuAdapter;

const INITIAL_DATA_SELECTOR: &str = "script#js-initialData";

impl SourceAdapter for ZhihuAdapter {
    fn id(&self) -> &'static str {
        "source.zhihu"
    }

    fn matches(&self, url: &reqwest::Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        (host == "zhihu.com" || host.ends_with(".zhihu.com")) && is_content_path(url)
    }

    fn capture<'a>(
        &'a self,
        client: &'a reqwest::Client,
        url: reqwest::Url,
        cookies: &'a crate::services::sources::cookies::SourceCookieStore,
    ) -> SourceFuture<'a> {
        Box::pin(async move {
            let cookie = cookies.cookie_for(&url).ok_or_else(|| {
                AppError::ParseFailed(
                    "zhihu needs a signed-in cookie: run `sources cookie set zhihu.com <cookie>`"
                        .to_string(),
                )
            })?;

            let response = client
                .get(url.as_str())
                .header(reqwest::header::USER_AGENT, browser_user_agent())
                .header(
                    reqwest::header::ACCEPT,
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9")
                .header(reqwest::header::REFERER, "https://www.zhihu.com/")
                .header(reqwest::header::COOKIE, cookie)
                .send()
                .await
                .map_err(map_source_request_error)?;

            if !response.status().is_success() {
                // Zhihu answers anonymous readers with a 403 anti-bot page, so a
                // failure here almost always means a missing or expired cookie.
                return Err(AppError::ParseFailed(format!(
                    "zhihu needs a signed-in cookie: request returned HTTP {}",
                    response.status().as_u16()
                )));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|_| AppError::ParseFailed("zhihu page could not be read".to_string()))?;
            let html = String::from_utf8_lossy(&bytes).to_string();

            parse_page(&html, url.as_str())?.map(Some).ok_or_else(|| {
                AppError::ParseFailed(
                    "zhihu body missing: the answer is not visible to this account, or the page structure changed"
                        .to_string(),
                )
            })
        })
    }
}

fn parse_page(html: &str, canonical_url: &str) -> AppResult<Option<SourceCapture>> {
    if let Some(capture) = parse_initial_data(html, canonical_url)? {
        return Ok(Some(capture));
    }

    parse_rendered_body(html, canonical_url)
}

/// `js-initialData` holds the full answer/article the client renders.
fn parse_initial_data(html: &str, canonical_url: &str) -> AppResult<Option<SourceCapture>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(INITIAL_DATA_SELECTOR).expect("static selector");

    let Some(script) = document.select(&selector).next() else {
        return Ok(None);
    };

    let raw: String = script.text().collect();
    let data: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };

    let entities = data
        .get("initialState")
        .and_then(|state| state.get("entities"));

    let Some(entities) = entities else {
        return Ok(None);
    };

    let (kind, id) = content_target(canonical_url)?;
    let entity = entities
        .get(kind)
        .and_then(|group| group.get(&id))
        .filter(|entity| entity.get("content").is_some());

    let Some(entity) = entity else {
        return Ok(None);
    };

    let content_html = entity
        .get("content")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let body = html_to_markdown(content_html)?;
    let title = question_title(entities, entity, kind);
    let author = entity
        .get("author")
        .and_then(|author| author.get("name"))
        .and_then(|value| value.as_str())
        .map(str::to_string);

    build_capture(canonical_url, title, author, &body, content_html, html)
}

fn question_title(
    entities: &serde_json::Value,
    entity: &serde_json::Value,
    kind: &str,
) -> Option<String> {
    if kind == "answers" {
        let question_id = entity.get("question")?.get("id")?.as_str()?;
        return entities
            .get("questions")?
            .get(question_id)?
            .get("title")?
            .as_str()
            .map(str::to_string);
    }

    entity
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

/// Rendered fallback for layouts without `js-initialData`.
fn parse_rendered_body(html: &str, canonical_url: &str) -> AppResult<Option<SourceCapture>> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("div.RichText").expect("static selector");

    let Some(body) = document.select(&body_selector).next() else {
        return Ok(None);
    };

    let title = document
        .select(&Selector::parse("h1").expect("static selector"))
        .next()
        .map(|node| node.text().collect::<String>().trim().to_string())
        .filter(|value| !value.is_empty());

    let content_html = body.inner_html();
    let body_markdown = html_to_markdown(&content_html)?;

    build_capture(
        canonical_url,
        title,
        None,
        &body_markdown,
        &content_html,
        html,
    )
}

fn build_capture(
    canonical_url: &str,
    title: Option<String>,
    author: Option<String>,
    body_markdown: &str,
    content_html: &str,
    raw_html: &str,
) -> AppResult<Option<SourceCapture>> {
    let mut assets = Vec::new();
    let fragment = Html::parse_fragment(content_html);
    let image_selector = Selector::parse("img").expect("static selector");

    for image in fragment.select(&image_selector) {
        let url = image
            .value()
            .attr("data-original")
            .or_else(|| image.value().attr("data-actualsrc"))
            .or_else(|| image.value().attr("src"))
            .filter(|value| value.starts_with("http"));

        if let Some(url) = url {
            assets.push(SourceAsset {
                kind: SourceAssetKind::Image,
                url: url.to_string(),
            });
        }
    }

    let markdown_content = render_markdown(title.as_deref(), Some(body_markdown), &assets);
    let text_content = markdown_to_text(&markdown_content);

    if text_content.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(SourceCapture {
        parser_id: "source.zhihu",
        canonical_url: canonical_url.to_string(),
        title,
        author,
        text_content,
        markdown_content,
        language: Some("zh-CN".to_string()),
        raw_payload: raw_html.to_string(),
        assets,
    }))
}

fn html_to_markdown(html: &str) -> AppResult<String> {
    if html.trim().is_empty() {
        return Ok(String::new());
    }

    let wrapped = format!("<html><body><article>{html}</article></body></html>");
    let parsed = parse_html_document(
        &wrapped,
        DocumentHints {
            title: None,
            author: None,
            description: None,
            language: Some("zh-CN".to_string()),
        },
    )?;

    Ok(parsed.markdown_content)
}

/// `(entity group, id)` for an answer or article URL.
fn content_target(url: &str) -> AppResult<(&'static str, String)> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::ParseFailed("zhihu url could not be parsed".to_string()))?;
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();

    if let Some(index) = segments.iter().position(|segment| *segment == "answer") {
        if let Some(id) = segments.get(index + 1) {
            if id.chars().all(|character| character.is_ascii_digit()) {
                return Ok(("answers", (*id).to_string()));
            }
        }
    }

    if let Some(index) = segments.iter().position(|segment| *segment == "p") {
        if let Some(id) = segments.get(index + 1) {
            if id.chars().all(|character| character.is_ascii_digit()) {
                return Ok(("articles", (*id).to_string()));
            }
        }
    }

    Err(AppError::ParseFailed(
        "zhihu url has no answer or article id".to_string(),
    ))
}

fn is_content_path(url: &reqwest::Url) -> bool {
    let path = url.path();
    path.contains("/answer/") || path.starts_with("/p/") || path.contains("/question/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const INITIAL_DATA_HTML: &str = r#"<html><head>
        <script id="js-initialData" type="text/json">{"initialState":{"entities":{"answers":{"2097140818339436319":{"id":"2097140818339436319","content":"<p>本地优先意味着断网时核心功能仍然可用。</p><p><img src=\"https://pic1.zhimg.com/v2-one.jpg\"></p><p>第二段正文。</p>","author":{"name":"李四"},"question":{"id":"62145775"}}},"questions":{"62145775":{"title":"如何评价本地优先软件？"}}}}}</script>
    </head><body></body></html>"#;

    const RENDERED_HTML: &str = r#"<html><body>
        <h1 class="QuestionHeader-title">如何评价本地优先软件？</h1>
        <div class="RichText">
            <p>本地优先意味着断网时核心功能仍然可用，写入先落到本机，联网后再合并。</p>
            <p>这样即便远端服务不可用，阅读、检索和整理仍然可以继续，不会因为网络问题丢掉内容。</p>
            <p><img data-original="https://pic1.zhimg.com/v2-one.jpg" src="https://pic1.zhimg.com/v2-one.jpg"></p>
        </div>
    </body></html>"#;

    const LOGIN_WALL_HTML: &str = r#"<html><body><div class="Modal">请登录</div></body></html>"#;

    fn answer_url() -> &'static str {
        "https://www.zhihu.com/question/62145775/answer/2097140818339436319"
    }

    #[test]
    fn matches_answer_and_article_paths() {
        let adapter = ZhihuAdapter;
        let answer = reqwest::Url::parse(answer_url()).unwrap();
        let article = reqwest::Url::parse("https://zhuanlan.zhihu.com/p/654321").unwrap();
        let home = reqwest::Url::parse("https://www.zhihu.com/").unwrap();

        assert!(adapter.matches(&answer));
        assert!(adapter.matches(&article));
        assert!(!adapter.matches(&home));
    }

    #[test]
    fn prefers_the_initial_data_payload() {
        let capture = parse_page(INITIAL_DATA_HTML, answer_url())
            .expect("parse should succeed")
            .expect("capture");

        assert_eq!(capture.title.as_deref(), Some("如何评价本地优先软件？"));
        assert_eq!(capture.author.as_deref(), Some("李四"));
        assert!(capture.text_content.contains("断网时核心功能仍然可用"));
        assert!(capture.text_content.contains("第二段正文"));
        assert_eq!(capture.assets.len(), 1);
    }

    #[test]
    fn falls_back_to_the_rendered_body() {
        let capture = parse_page(RENDERED_HTML, answer_url())
            .expect("parse should succeed")
            .expect("capture");

        assert_eq!(capture.title.as_deref(), Some("如何评价本地优先软件？"));
        assert!(capture.text_content.contains("断网时核心功能仍然可用"));
        assert!(capture.text_content.contains("阅读、检索和整理"));
        assert_eq!(capture.assets.len(), 1);
    }

    #[test]
    fn returns_none_for_login_wall_instead_of_inventing_text() {
        assert!(parse_page(LOGIN_WALL_HTML, answer_url())
            .expect("parse should succeed")
            .is_none());
    }

    #[test]
    fn extracts_answer_and_article_targets() {
        assert_eq!(
            content_target(answer_url()).expect("target"),
            ("answers", "2097140818339436319".to_string())
        );
        assert_eq!(
            content_target("https://zhuanlan.zhihu.com/p/654321").expect("target"),
            ("articles", "654321".to_string())
        );
    }
}
