//! Zhihu (知乎) adapter.
//!
//! Answers and articles keep their body in `div.RichText`, but the mobile page is
//! far lighter than the desktop shell and renders that body server-side. The
//! adapter extracts that fragment, converts it with the shared HTML parser and
//! keeps the original image URLs in order.
//!
//! Login-walled (盐选) content returns an empty body; the caller then falls back to
//! the generic pipeline, which reports the login wall instead of inventing text.

use super::{
    markdown_to_text, mobile_user_agent, render_markdown, SourceAdapter, SourceAsset,
    SourceAssetKind, SourceCapture, SourceFuture,
};
use crate::errors::{AppError, AppResult};
use crate::services::document_parser::{parse_html_document, DocumentHints};
use scraper::{Html, Selector};

pub struct ZhihuAdapter;

impl SourceAdapter for ZhihuAdapter {
    fn id(&self) -> &'static str {
        "source.zhihu"
    }

    fn matches(&self, url: &reqwest::Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        (host == "zhihu.com" || host.ends_with(".zhihu.com")) && is_content_path(url)
    }

    fn capture<'a>(&'a self, client: &'a reqwest::Client, url: reqwest::Url) -> SourceFuture<'a> {
        Box::pin(async move {
            let response = client
                .get(url.as_str())
                .header(reqwest::header::USER_AGENT, mobile_user_agent())
                .header(
                    reqwest::header::ACCEPT,
                    "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8",
                )
                .send()
                .await
                .map_err(|_| AppError::ParseFailed("zhihu request failed".to_string()))?;

            if !response.status().is_success() {
                return Err(AppError::ParseFailed(
                    "zhihu content unavailable".to_string(),
                ));
            }

            let html = response
                .text()
                .await
                .map_err(|_| AppError::ParseFailed("zhihu page was not text".to_string()))?;

            parse_page(&html, url.as_str())?.map(Some).ok_or_else(|| {
                AppError::ParseFailed(
                    "zhihu body missing (login wall or structure change)".to_string(),
                )
            })
        })
    }
}

fn parse_page(html: &str, canonical_url: &str) -> AppResult<Option<SourceCapture>> {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("div.RichText").expect("static selector");
    let title_selector = Selector::parse("h1").expect("static selector");
    let image_selector = Selector::parse("img").expect("static selector");

    let body = match document.select(&body_selector).next() {
        Some(body) => body,
        None => return Ok(None),
    };

    let title = document
        .select(&title_selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_string())
        .filter(|value| !value.is_empty());

    let author = document
        .select(&Selector::parse("meta[name=\"author\"]").expect("static selector"))
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(str::to_string);

    let body_html = body.inner_html();
    let wrapped = format!("<html><body><article>{body_html}</article></body></html>");
    let parsed = parse_html_document(
        &wrapped,
        DocumentHints {
            title: title.clone(),
            author: author.clone(),
            description: None,
            language: Some("zh-CN".to_string()),
        },
    )?;

    let mut assets = Vec::new();
    for image in body.select(&image_selector) {
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

    let markdown_content =
        render_markdown(title.as_deref(), Some(&parsed.markdown_content), &assets);
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
        raw_payload: html.to_string(),
        assets,
    }))
}

fn is_content_path(url: &reqwest::Url) -> bool {
    let path = url.path();
    path.contains("/answer/") || path.starts_with("/p/") || path.contains("/question/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER_HTML: &str = r#"<html><head>
        <meta name="author" content="李四">
    </head><body>
        <h1 class="QuestionHeader-title">如何评价本地优先软件？</h1>
        <div class="RichText">
            <p>本地优先意味着断网时核心功能仍然可用。</p>
            <p><img src="https://pic1.zhimg.com/v2-one.jpg" data-original="https://pic1.zhimg.com/v2-one.jpg"></p>
            <p>第二段正文。</p>
        </div>
    </body></html>"#;

    const LOGIN_WALL_HTML: &str = r#"<html><body><div class="Modal">请登录</div></body></html>"#;

    #[test]
    fn matches_answer_and_article_paths() {
        let adapter = ZhihuAdapter;
        let answer = reqwest::Url::parse("https://www.zhihu.com/question/123/answer/456").unwrap();
        let article = reqwest::Url::parse("https://zhuanlan.zhihu.com/p/654321").unwrap();
        let home = reqwest::Url::parse("https://www.zhihu.com/").unwrap();

        assert!(adapter.matches(&answer));
        assert!(adapter.matches(&article));
        assert!(!adapter.matches(&home));
    }

    #[test]
    fn parses_body_title_author_and_images() {
        let capture = parse_page(ANSWER_HTML, "https://www.zhihu.com/question/1/answer/2")
            .expect("parse should succeed")
            .expect("capture");

        assert_eq!(capture.title.as_deref(), Some("如何评价本地优先软件？"));
        assert_eq!(capture.author.as_deref(), Some("李四"));
        assert!(capture.text_content.contains("断网时核心功能仍然可用"));
        assert!(capture
            .markdown_content
            .contains("![image](https://pic1.zhimg.com/v2-one.jpg)"));
    }

    #[test]
    fn returns_none_for_login_wall_instead_of_inventing_text() {
        assert!(
            parse_page(LOGIN_WALL_HTML, "https://www.zhihu.com/question/1/answer/2")
                .expect("parse should succeed")
                .is_none()
        );
    }
}
