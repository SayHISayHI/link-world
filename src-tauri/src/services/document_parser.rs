use crate::errors::{AppError, AppResult};
use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};

const MIN_EXTRACTED_TEXT_CHARS: usize = 24;
const READABLE_CONTAINER_SELECTORS: &[&str] = &[
    r#"[itemprop="articleBody"]"#,
    "#js_content",
    ".rich_media_content",
    ".topic_content",
    ".markdown_body",
    ".markdown-body",
    ".entry-content",
    ".article-content",
    ".post-content",
    ".Post-RichText",
    ".RichContent-inner",
    r#"[data-testid="tweetText"]"#,
    "article",
    ".article",
    ".post",
    ".content",
    "main",
    r#"[role="main"]"#,
    "body",
];

#[derive(Debug, Clone, Default)]
pub struct DocumentHints {
    pub title: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedWebDocument {
    pub title: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub text_content: String,
    pub markdown_content: String,
}

pub fn parse_html_document(html: &str, hints: DocumentHints) -> AppResult<ParsedWebDocument> {
    let document = Html::parse_document(html);
    let title = normalize_optional(hints.title).or_else(|| select_document_title(&document));
    let author = normalize_optional(hints.author).or_else(|| select_document_author(&document));
    let description = normalize_optional(hints.description).or_else(|| {
        select_first_attr(&document, r#"meta[name="description"]"#, "content").or_else(|| {
            select_first_attr(&document, r#"meta[property="og:description"]"#, "content")
        })
    });
    let language =
        normalize_optional(hints.language).or_else(|| select_first_attr(&document, "html", "lang"));

    let root = select_readable_root(&document);
    let (mut text_content, mut markdown_content) = if let Some(root) = root {
        (
            normalize_block_output(&render_plain_children(root)),
            normalize_block_output(&render_markdown_children(root, 0)),
        )
    } else {
        let fallback = description.clone().unwrap_or_else(|| strip_html_tags(html));
        let fallback = normalize_whitespace(&fallback);
        (fallback.clone(), fallback)
    };

    if let Some(title) = title.as_deref() {
        text_content = strip_leading_plain_title(&text_content, title);
        markdown_content = strip_leading_markdown_title(&markdown_content, title);
    }

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

    if markdown_content.is_empty() {
        markdown_content = text_content.clone();
    }

    Ok(ParsedWebDocument {
        title,
        author,
        description,
        language,
        text_content,
        markdown_content,
    })
}

fn select_readable_root<'a>(document: &'a Html) -> Option<ElementRef<'a>> {
    let mut best: Option<(usize, ElementRef<'a>)> = None;

    for selector_text in READABLE_CONTAINER_SELECTORS {
        let Ok(selector) = Selector::parse(selector_text) else {
            continue;
        };

        for element in document.select(&selector) {
            let text = normalize_block_output(&render_plain_children(element));
            if text.is_empty() {
                continue;
            }

            let score = readable_root_score(element, selector_text, &text);
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, element));
            }
        }
    }

    best.map(|(_, element)| element)
}

fn readable_root_score(element: ElementRef<'_>, selector: &str, text: &str) -> usize {
    let structure_bonus = count_matches(element, "p") * 120
        + count_matches(element, "h1, h2, h3, h4, h5, h6") * 180
        + count_matches(element, "pre") * 250
        + count_matches(element, "li") * 30;
    readable_container_bonus(selector)
        + structure_bonus
        + text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count()
}

fn readable_container_bonus(selector: &str) -> usize {
    match selector {
        r#"[itemprop="articleBody"]"# => 50_000,
        "#js_content"
        | ".rich_media_content"
        | ".topic_content"
        | ".markdown_body"
        | ".markdown-body"
        | ".entry-content"
        | ".article-content"
        | ".post-content"
        | ".Post-RichText"
        | ".RichContent-inner"
        | r#"[data-testid="tweetText"]"# => 40_000,
        "article" => 30_000,
        ".article" | ".post" | ".content" => 20_000,
        "main" | r#"[role="main"]"# => 10_000,
        _ => 0,
    }
}

fn count_matches(element: ElementRef<'_>, selector: &str) -> usize {
    Selector::parse(selector)
        .ok()
        .map(|selector| element.select(&selector).count())
        .unwrap_or_default()
}

fn render_plain_children(element: ElementRef<'_>) -> String {
    element
        .children()
        .map(render_plain_node)
        .collect::<Vec<_>>()
        .join("")
}

fn render_plain_node(node: NodeRef<'_, Node>) -> String {
    match node.value() {
        Node::Text(text) => normalize_inline_text(text),
        Node::Element(_) => {
            let Some(element) = ElementRef::wrap(node) else {
                return String::new();
            };
            let tag = element.value().name();
            if is_ignored_tag(tag) {
                return String::new();
            }

            match tag {
                "br" => "\n".to_string(),
                "pre" => format!("\n\n{}\n\n", normalized_pre_text(element)),
                "table" => format!("\n\n{}\n\n", table_to_plain_text(element)),
                "li" => format!(
                    "\n- {}\n",
                    normalize_block_output(&render_plain_children(element))
                ),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "blockquote" | "figcaption"
                | "address" | "article" | "aside" | "div" | "dl" | "fieldset" | "figure"
                | "header" | "main" | "ol" | "section" | "ul" => {
                    format!("\n\n{}\n\n", render_plain_children(element))
                }
                "hr" => "\n\n---\n\n".to_string(),
                _ => render_plain_children(element),
            }
        }
        _ => String::new(),
    }
}

fn render_markdown_children(element: ElementRef<'_>, list_depth: usize) -> String {
    element
        .children()
        .map(|node| render_markdown_node(node, list_depth))
        .collect::<Vec<_>>()
        .join("")
}

fn render_markdown_node(node: NodeRef<'_, Node>, list_depth: usize) -> String {
    match node.value() {
        Node::Text(text) => escape_markdown_text(&normalize_inline_text(text)),
        Node::Element(_) => {
            let Some(element) = ElementRef::wrap(node) else {
                return String::new();
            };
            let tag = element.value().name();
            if is_ignored_tag(tag) {
                return String::new();
            }

            match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag[1..].parse::<usize>().unwrap_or(1);
                    let content =
                        normalize_inline_markdown(&render_markdown_children(element, list_depth));
                    format!("\n\n{} {}\n\n", "#".repeat(level), content)
                }
                "p" | "figcaption" => format!(
                    "\n\n{}\n\n",
                    normalize_inline_markdown(&render_markdown_children(element, list_depth))
                ),
                "br" => "  \n".to_string(),
                "strong" | "b" => format!(
                    "**{}**",
                    normalize_inline_markdown(&render_markdown_children(element, list_depth))
                ),
                "em" | "i" => format!(
                    "_{}_",
                    normalize_inline_markdown(&render_markdown_children(element, list_depth))
                ),
                "del" | "s" => format!(
                    "~~{}~~",
                    normalize_inline_markdown(&render_markdown_children(element, list_depth))
                ),
                "code" => inline_code(&element.text().collect::<String>()),
                "pre" => pre_to_markdown(element),
                "blockquote" => blockquote_to_markdown(element, list_depth),
                "ul" => list_to_markdown(element, false, list_depth),
                "ol" => list_to_markdown(element, true, list_depth),
                "li" => render_markdown_children(element, list_depth),
                "a" => link_to_markdown(element, list_depth),
                "img" => image_to_markdown(element),
                "table" => table_to_markdown(element),
                "hr" => "\n\n---\n\n".to_string(),
                "article" | "aside" | "div" | "figure" | "header" | "main" | "section" => {
                    format!("\n\n{}\n\n", render_markdown_children(element, list_depth))
                }
                _ => render_markdown_children(element, list_depth),
            }
        }
        _ => String::new(),
    }
}

fn list_to_markdown(element: ElementRef<'_>, ordered: bool, list_depth: usize) -> String {
    let indentation = "  ".repeat(list_depth);
    let items = element
        .child_elements()
        .filter(|child| child.value().name() == "li")
        .enumerate()
        .map(|(index, item)| {
            let prefix = if ordered {
                format!("{}. ", index + 1)
            } else {
                "- ".to_string()
            };
            let content = normalize_block_output(&render_markdown_children(item, list_depth + 1));
            let continuation = format!("\n{}  ", indentation);
            format!(
                "{}{}{}",
                indentation,
                prefix,
                content.replace('\n', &continuation)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n{}\n", items)
}

fn blockquote_to_markdown(element: ElementRef<'_>, list_depth: usize) -> String {
    let content = normalize_block_output(&render_markdown_children(element, list_depth));
    let quoted = content
        .lines()
        .map(|line| {
            if line.is_empty() {
                ">".to_string()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n\n{quoted}\n\n")
}

fn pre_to_markdown(element: ElementRef<'_>) -> String {
    let code_selector = Selector::parse("code").expect("static code selector should parse");
    let code = element.select(&code_selector).next();
    let language = code
        .and_then(|code| {
            code.value().classes().find_map(|class| {
                class
                    .strip_prefix("language-")
                    .or_else(|| class.strip_prefix("lang-"))
            })
        })
        .unwrap_or_default();
    let value = code
        .map(|code| normalized_pre_text(code))
        .unwrap_or_else(|| normalized_pre_text(element));
    let fence = if value.contains("```") { "````" } else { "```" };
    format!("\n\n{fence}{language}\n{value}\n{fence}\n\n")
}

fn inline_code(value: &str) -> String {
    let value = normalize_whitespace(value);
    let fence_length = longest_backtick_run(&value) + 1;
    let fence = "`".repeat(fence_length.max(1));
    format!("{fence}{value}{fence}")
}

fn longest_backtick_run(value: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for character in value.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn link_to_markdown(element: ElementRef<'_>, list_depth: usize) -> String {
    let label = normalize_inline_markdown(&render_markdown_children(element, list_depth));
    let Some(href) = element.attr("href").filter(|href| is_safe_url(href)) else {
        return label;
    };
    format!("[{label}]({})", href.replace(')', "%29"))
}

fn image_to_markdown(element: ElementRef<'_>) -> String {
    let Some(src) = element.attr("src").filter(|src| is_safe_url(src)) else {
        return String::new();
    };
    let alt = element.attr("alt").unwrap_or_default().replace(']', "\\]");
    format!("![{alt}]({})", src.replace(')', "%29"))
}

fn is_safe_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with('/')
        || lower.starts_with('#')
}

fn table_to_plain_text(table: ElementRef<'_>) -> String {
    table_rows(table)
        .into_iter()
        .map(|row| row.join(" | "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn table_to_markdown(table: ElementRef<'_>) -> String {
    let rows = table_rows(table);
    if rows.is_empty() {
        return String::new();
    }

    let width = rows.iter().map(Vec::len).max().unwrap_or_default();
    if width == 0 {
        return String::new();
    }

    let normalize_row = |row: &[String]| {
        let mut cells = row.to_vec();
        cells.resize(width, String::new());
        format!("| {} |", cells.join(" | "))
    };
    let mut lines = vec![normalize_row(&rows[0])];
    lines.push(format!("| {} |", vec!["---"; width].join(" | ")));
    lines.extend(rows.iter().skip(1).map(|row| normalize_row(row)));
    format!("\n\n{}\n\n", lines.join("\n"))
}

fn table_rows(table: ElementRef<'_>) -> Vec<Vec<String>> {
    let row_selector = Selector::parse("tr").expect("static row selector should parse");
    let cell_selector =
        Selector::parse(":scope > th, :scope > td").expect("static cell selector should parse");
    table
        .select(&row_selector)
        .map(|row| {
            row.select(&cell_selector)
                .map(|cell| {
                    normalize_whitespace(&cell.text().collect::<Vec<_>>().join(" "))
                        .replace('|', "\\|")
                })
                .collect::<Vec<_>>()
        })
        .filter(|row| !row.is_empty())
        .collect()
}

fn normalized_pre_text(element: ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn is_ignored_tag(tag: &str) -> bool {
    matches!(
        tag,
        "script"
            | "style"
            | "noscript"
            | "template"
            | "iframe"
            | "canvas"
            | "svg"
            | "video"
            | "audio"
            | "form"
            | "input"
            | "button"
            | "nav"
            | "footer"
    )
}

fn normalize_inline_text(value: &str) -> String {
    let has_leading_space = value.chars().next().is_some_and(char::is_whitespace);
    let has_trailing_space = value.chars().next_back().is_some_and(char::is_whitespace);
    let normalized = normalize_whitespace(value);
    if normalized.is_empty() {
        return if value.chars().any(char::is_whitespace) {
            " ".to_string()
        } else {
            String::new()
        };
    }

    format!(
        "{}{}{}",
        if has_leading_space { " " } else { "" },
        normalized,
        if has_trailing_space { " " } else { "" }
    )
}

fn normalize_inline_markdown(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_block_output(value: &str) -> String {
    let mut lines = Vec::new();
    let mut previous_blank = true;

    for raw_line in value.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            if !previous_blank {
                lines.push(String::new());
                previous_blank = true;
            }
        } else {
            lines.push(line.to_string());
            previous_blank = false;
        }
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}

fn strip_leading_plain_title(value: &str, title: &str) -> String {
    let mut lines = value.lines();
    if lines.next().is_some_and(|line| line.trim() == title.trim()) {
        return lines.collect::<Vec<_>>().join("\n").trim().to_string();
    }
    value.to_string()
}

fn strip_leading_markdown_title(value: &str, title: &str) -> String {
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let heading = first.trim_start_matches('#').trim();
    if first.starts_with('#') && heading == title.trim() {
        return lines.collect::<Vec<_>>().join("\n").trim().to_string();
    }
    value.to_string()
}

fn escape_markdown_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn select_document_title(document: &Html) -> Option<String> {
    select_first_attr(document, r#"meta[itemprop="headline"]"#, "content")
        .or_else(|| select_first_attr(document, r#"meta[property="og:title"]"#, "content"))
        .or_else(|| select_first_text(document, r#"h1[itemprop="headline"]"#))
        .or_else(|| select_first_text(document, "article h1"))
        .or_else(|| select_first_text(document, "h1"))
        .or_else(|| select_first_text(document, "title"))
}

fn select_document_author(document: &Html) -> Option<String> {
    select_first_attr(
        document,
        r#"[itemprop="author"] [itemprop="name"]"#,
        "content",
    )
    .or_else(|| select_first_attr(document, r#"meta[name="author"]"#, "content"))
    .or_else(|| select_first_attr(document, r#"meta[property="article:author"]"#, "content"))
    .or_else(|| select_first_text(document, r#"[rel="author"]"#))
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

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| normalize_whitespace(&value))
        .filter(|value| !value.is_empty())
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

    normalize_whitespace(&output)
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
    let metadata = format!(
        "{} {}",
        title.unwrap_or_default(),
        description.unwrap_or_default()
    );
    let lower_metadata = metadata.to_lowercase();
    let lower_text = text_content.to_lowercase();
    let text_chars = text_content
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    let is_short_page = text_chars < 1_200;

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
        if lower_metadata.contains(marker) || (is_short_page && lower_text.contains(marker)) {
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
        if metadata.contains(marker) || (is_short_page && text_content.contains(marker)) {
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

#[cfg(test)]
mod tests {
    use super::{parse_html_document, DocumentHints};

    #[test]
    fn parses_schema_article_into_structured_text_and_markdown() {
        let parsed = parse_html_document(
            r#"<!doctype html>
            <html lang="zh-CN">
              <head>
                <title>Clean title plus page suffix - Example</title>
                <meta itemprop="headline" content="Clean title">
                <meta name="description" content="A compact article description.">
              </head>
              <body>
                <article>
                  <div itemprop="author"><meta itemprop="name" content="Example Author"></div>
                  <h1>Clean title</h1>
                  <div class="author-info"><p>42 reads and unrelated metadata</p></div>
                  <div itemprop="articleBody">
                    <blockquote><p>Boundary-aware intro.</p></blockquote>
                    <h2>First section</h2>
                    <p>Paragraph with <strong>important text</strong> and <code>inline_code()</code>.</p>
                    <pre><code class="language-js">const answer = 42;
return answer;</code></pre>
                    <table>
                      <tr><th>Capability</th><th>Result</th></tr>
                      <tr><td>Structure</td><td>Preserved</td></tr>
                    </table>
                  </div>
                </article>
              </body>
            </html>"#,
            DocumentHints::default(),
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
        assert!(parsed.markdown_content.contains("> Boundary-aware intro."));
        assert!(parsed.markdown_content.contains("## First section"));
        assert!(parsed.markdown_content.contains("**important text**"));
        assert!(parsed.markdown_content.contains("`inline_code()`"));
        assert!(parsed
            .markdown_content
            .contains("```js\nconst answer = 42;\nreturn answer;\n```"));
        assert!(parsed
            .markdown_content
            .contains("| Capability | Result |\n| --- | --- |"));
    }

    #[test]
    fn uses_capture_hints_for_sanitized_dom_fragments() {
        let parsed = parse_html_document(
            "<div itemprop=\"articleBody\"><h2>Section</h2><p>Captured body with enough meaningful content.</p></div>",
            DocumentHints {
                title: Some("Captured title".to_string()),
                author: Some("Captured author".to_string()),
                description: None,
                language: Some("en".to_string()),
            },
        )
        .expect("DOM fragment should parse");

        assert_eq!(parsed.title.as_deref(), Some("Captured title"));
        assert_eq!(parsed.author.as_deref(), Some("Captured author"));
        assert_eq!(
            parsed.markdown_content,
            "## Section\n\nCaptured body with enough meaningful content."
        );
    }

    #[test]
    fn rejects_verification_pages() {
        let error = parse_html_document(
            "<html><head><title>环境异常</title></head><body><main>环境异常，完成验证后继续访问。去验证</main></body></html>",
            DocumentHints::default(),
        )
        .expect_err("verification page should be rejected");

        assert!(error.to_string().contains("verification page"));
    }

    #[test]
    fn keeps_long_articles_that_discuss_verification_topics() {
        let body = format!(
            "This guide explains captcha handling as an engineering topic. {}",
            "It provides implementation context, tradeoffs, examples, and operational guidance. "
                .repeat(30)
        );
        let parsed = parse_html_document(
            &format!(
                "<html><head><title>Authentication engineering guide</title></head><body><article><p>{body}</p></article></body></html>"
            ),
            DocumentHints::default(),
        )
        .expect("a substantive article should not be mistaken for a challenge page");

        assert!(parsed.text_content.contains("captcha handling"));
    }
}
