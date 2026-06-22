use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use std::{borrow::Cow, collections::HashSet, ops::Range};
use uuid::Uuid;

const ALLOWED_STYLE_PROPERTIES: [&str; 34] = [
    "align-items",
    "background-color",
    "border",
    "border-radius",
    "color",
    "display",
    "flex-direction",
    "flex-wrap",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "gap",
    "height",
    "justify-content",
    "letter-spacing",
    "line-height",
    "margin",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "overflow",
    "overflow-x",
    "overflow-y",
    "padding",
    "text-align",
    "text-decoration",
    "text-indent",
    "text-transform",
    "vertical-align",
    "white-space",
    "width",
    "word-spacing",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedMarkdown {
    pub html: String,
    pub has_mermaid: bool,
    pub has_math: bool,
}

pub fn render_markdown(input: &str, session_id: Option<Uuid>) -> RenderedMarkdown {
    let stripped = strip_frontmatter(input);
    let input = stripped.content;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);

    let has_math = Parser::new_ext(input, options)
        .any(|event| matches!(event, Event::InlineMath(_) | Event::DisplayMath(_)));
    let parser = Parser::new_ext(input, options)
        .into_offset_iter()
        .flat_map(|(event, range)| {
            render_events(event, range, input, stripped.line_offset, session_id)
        });
    let mut html = String::new();
    html::push_html(&mut html, parser);
    let html = sanitize_rendered_html(&html);

    RenderedMarkdown {
        html,
        has_mermaid: has_mermaid_fence(input),
        has_math,
    }
}

fn render_events<'a>(
    event: Event<'a>,
    range: Range<usize>,
    input: &str,
    line_offset: usize,
    session_id: Option<Uuid>,
) -> Vec<Event<'a>> {
    let mut events = Vec::with_capacity(2);
    if should_anchor(&event) {
        events.push(source_anchor(source_line(input, range.start, line_offset)));
    }
    events.push(render_event(event, session_id));
    events
}

fn render_event(event: Event<'_>, session_id: Option<Uuid>) -> Event<'_> {
    match event {
        Event::InlineMath(math) => Event::Html(CowStr::Boxed(
            format!(
                r#"<span class="math math-inline">{}</span>"#,
                escape_html_text(&math)
            )
            .into_boxed_str(),
        )),
        Event::DisplayMath(math) => Event::Html(CowStr::Boxed(
            format!(
                r#"<div class="math math-display">{}</div>"#,
                escape_html_text(&math)
            )
            .into_boxed_str(),
        )),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let dest_url = session_id
                .and_then(|id| image_api_url(id, &dest_url))
                .map(CowStr::Boxed)
                .unwrap_or(dest_url);
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            })
        }
        other => other,
    }
}

fn sanitize_rendered_html(input: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(&[
            "address", "font", "input", "main", "meter", "progress", "section", "tfoot",
        ])
        .add_tag_attributes("font", &["color"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .add_tag_attributes("meter", &["value", "min", "max", "low", "high", "optimum"])
        .add_tag_attributes("progress", &["value", "max"])
        .add_generic_attributes(&["class", "id", "style"])
        .add_generic_attribute_prefixes(&["data-"])
        .attribute_filter(|_, attribute, value| {
            (attribute == "style")
                .then(|| Cow::Owned(normalize_style_property_names(value)))
                .or(Some(Cow::Borrowed(value)))
        })
        .filter_style_properties(HashSet::from(ALLOWED_STYLE_PROPERTIES));

    builder.clean(input).to_string()
}

fn normalize_style_property_names(style: &str) -> String {
    style
        .split(';')
        .map(|declaration| {
            declaration.split_once(':').map_or_else(
                || declaration.to_owned(),
                |(property, value)| format!("{}:{value}", property.to_ascii_lowercase()),
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[derive(Debug, Clone, Copy)]
struct StrippedMarkdown<'a> {
    content: &'a str,
    line_offset: usize,
}

fn strip_frontmatter(input: &str) -> StrippedMarkdown<'_> {
    let Some(after_open) = input.strip_prefix("---") else {
        return StrippedMarkdown {
            content: input,
            line_offset: 0,
        };
    };
    if !after_open.starts_with('\n') && !after_open.starts_with("\r\n") {
        return StrippedMarkdown {
            content: input,
            line_offset: 0,
        };
    }

    let mut offset = 3;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        offset += line.len();
        if trimmed == "---" || trimmed == "..." {
            return StrippedMarkdown {
                content: &input[offset..],
                line_offset: input[..offset]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count(),
            };
        }
    }

    StrippedMarkdown {
        content: input,
        line_offset: 0,
    }
}

fn should_anchor(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::Start(
            Tag::Paragraph
                | Tag::Heading { .. }
                | Tag::BlockQuote(_)
                | Tag::CodeBlock(_)
                | Tag::List(_)
                | Tag::FootnoteDefinition(_)
                | Tag::DefinitionList
                | Tag::Table(_)
        )
    )
}

fn source_anchor(line: usize) -> Event<'static> {
    Event::Html(CowStr::Boxed(
        format!(r#"<span class="mdlive-source-anchor" data-source-line="{line}"></span>"#)
            .into_boxed_str(),
    ))
}

fn source_line(input: &str, byte_offset: usize, line_offset: usize) -> usize {
    line_offset
        + input[..byte_offset]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
        + 1
}

fn has_mermaid_fence(input: &str) -> bool {
    input.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("```")
            .or_else(|| trimmed.strip_prefix("~~~"))
        else {
            return false;
        };
        rest.trim_start()
            .split(|ch: char| ch.is_whitespace() || ch == '{')
            .next()
            .is_some_and(|lang| lang.eq_ignore_ascii_case("mermaid"))
    })
}

fn image_api_url(session_id: Uuid, dest_url: &str) -> Option<Box<str>> {
    if dest_url.is_empty()
        || dest_url.starts_with('/')
        || dest_url.starts_with('#')
        || dest_url.starts_with("data:")
        || dest_url.contains("://")
    {
        return None;
    }

    Some(format!("/api/image/{session_id}/{dest_url}").into_boxed_str())
}

fn escape_html_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_markdown_extensions() {
        let rendered = render_markdown(
            "- [x] done\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n~~old~~",
            None,
        );

        assert!(rendered.html.contains("type=\"checkbox\""));
        assert!(rendered.html.contains("<table>"));
        assert!(rendered.html.contains("<del>old</del>"));
    }

    #[test]
    fn adds_source_line_anchors_to_block_elements() {
        let rendered = render_markdown("# Heading\n\nbody", None);

        assert!(rendered.html.contains(r#"data-source-line="1""#));
        assert!(rendered.html.contains(r#"data-source-line="3""#));
    }

    #[test]
    fn source_line_anchors_preserve_original_lines_after_frontmatter() {
        let rendered = render_markdown("---\ntitle: A\n---\n# Heading", None);

        assert!(rendered.html.contains(r#"data-source-line="4""#));
    }

    #[test]
    fn detects_backtick_mermaid_fence() {
        let rendered = render_markdown("```mermaid\ngraph TD\nA-->B\n```", None);

        assert!(rendered.has_mermaid);
    }

    #[test]
    fn detects_tilde_mermaid_fence_with_attributes() {
        let rendered = render_markdown("~~~ mermaid {#chart}\ngraph TD\nA-->B\n~~~", None);

        assert!(rendered.has_mermaid);
    }

    #[test]
    fn renders_safe_raw_html() {
        let rendered = render_markdown(
            "<script>alert(1)</script>\n\n<div><b>block</b></div>\n\n<details><summary>More</summary>Text</details>\n\nHello <b>world</b> and <font color=\"red\">red</font>",
            None,
        );

        assert!(rendered.html.contains("<div><b>block</b></div>"));
        assert!(
            rendered
                .html
                .contains("<details><summary>More</summary>Text</details>")
        );
        assert!(rendered.html.contains("Hello <b>world</b>"));
        assert!(rendered.html.contains(r#"<font color="red">red</font>"#));
        assert!(!rendered.html.contains("<script>"));
        assert!(!rendered.html.contains("alert(1)"));
    }

    #[test]
    fn strips_yaml_frontmatter() {
        let rendered = render_markdown("---\ntitle: A\n---\n# Heading", None);

        assert!(!rendered.html.contains("title: A"));
        assert!(rendered.html.contains("<h1>Heading</h1>"));
    }

    #[test]
    fn keeps_horizontal_rule_without_frontmatter_close() {
        let rendered = render_markdown("---\n\nbody", None);

        assert!(rendered.html.contains("<hr"));
        assert!(rendered.html.contains("body"));
    }

    #[test]
    fn renders_math_markers_for_katex() {
        let rendered = render_markdown("Inline $a < b$.\n\n$$c > d$$", None);

        assert!(rendered.has_math);
        assert!(
            rendered
                .html
                .contains(r#"<span class="math math-inline">a &lt; b</span>"#)
        );
        assert!(
            rendered
                .html
                .contains(r#"<div class="math math-display">c &gt; d</div>"#)
        );
    }

    #[test]
    fn rewrites_relative_image_urls_for_session() {
        let session_id = Uuid::new_v4();
        let rendered = render_markdown(
            "![local](<images/a b.png>)\n\n![remote](https://example.test/a.png)",
            Some(session_id),
        );

        assert!(rendered.html.contains(&format!(
            r#"src="/api/image/{session_id}/images/a%20b.png""#
        )));
        assert!(
            rendered
                .html
                .contains(r#"src="https://example.test/a.png""#)
        );
    }

    #[test]
    fn sanitizer_removes_dangerous_urls_and_event_handlers() {
        let input = r#"<a href="javascript:alert(1)" onclick="alert(1)">x</a><img src="javascript:alert(1)" onerror="alert(1)"><font color="red" face="serif" onclick="alert(1)">red</font>"#;
        let html = sanitize_rendered_html(input);

        assert!(html.contains(r#"<font color="red">red</font>"#));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("onclick"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("face="));
    }

    #[test]
    fn sanitizer_filters_inline_styles() {
        let input = r#"<section style="color: red; background-color: #fff; margin: 1rem; display: flex; gap: 8px; position: fixed; z-index: 999; background-image: url(https://example.test/tracker.png); transform: scale(2)">styled</section>"#;
        let html = sanitize_rendered_html(input);

        assert!(html.contains("color:red"));
        assert!(html.contains("background-color:#fff"));
        assert!(html.contains("margin:1rem"));
        assert!(html.contains("display:flex"));
        assert!(html.contains("gap:8px"));
        assert!(!html.contains("position"));
        assert!(!html.contains("z-index"));
        assert!(!html.contains("background-image"));
        assert!(!html.contains("example.test"));
        assert!(!html.contains("transform"));
    }

    #[test]
    fn sanitizer_matches_inline_style_properties_case_insensitively() {
        let input =
            r#"<section style="COLOR: red; Font-Weight: bold; POSITION: fixed">styled</section>"#;
        let html = sanitize_rendered_html(input);

        assert!(html.contains("color:red"));
        assert!(html.contains("font-weight:bold"));
        assert!(!html.contains("position"));
    }

    #[test]
    fn sanitizer_keeps_safe_static_elements_and_attributes() {
        let input = r#"<main><section><address>Somewhere</address><meter value="0.6" min="0" max="1" low="0.2" high="0.8" optimum="0.7" onclick="alert(1)">60%</meter><progress value="3" max="10" formaction="https://example.test">3/10</progress><table><tfoot><tr><td>Footer</td></tr></tfoot></table></section></main>"#;
        let html = sanitize_rendered_html(input);

        assert!(html.contains("<main><section><address>Somewhere</address>"));
        assert!(html.contains(
            r#"<meter value="0.6" min="0" max="1" low="0.2" high="0.8" optimum="0.7">60%</meter>"#
        ));
        assert!(html.contains(r#"<progress value="3" max="10">3/10</progress>"#));
        assert!(html.contains("<tfoot><tr><td>Footer</td></tr></tfoot>"));
        assert!(!html.contains("onclick"));
        assert!(!html.contains("formaction"));
        assert!(!html.contains("example.test"));
    }

    #[test]
    fn sanitizer_keeps_markdown_generated_attributes() {
        let session_id = Uuid::new_v4();
        let rendered = render_markdown(
            "- [x] done\n\n```mermaid\ngraph TD\nA-->B\n```\n\nInline $a$.\n\nnote[^1]\n\n[^1]: footnote\n\n![local](images/a.png)",
            Some(session_id),
        );

        assert!(rendered.html.contains(r#"type="checkbox""#));
        assert!(rendered.html.contains(r#"checked"#));
        assert!(rendered.html.contains(r#"class="language-mermaid""#));
        assert!(rendered.html.contains(r#"class="math math-inline""#));
        assert!(rendered.html.contains(r#"class="mdlive-source-anchor""#));
        assert!(rendered.html.contains(r#"data-source-line="1""#));
        assert!(rendered.html.contains(r##"href="#1""##));
        assert!(rendered.html.contains(r#"id="1""#));
        assert!(
            rendered
                .html
                .contains(&format!(r#"src="/api/image/{session_id}/images/a.png""#))
        );
    }
}
