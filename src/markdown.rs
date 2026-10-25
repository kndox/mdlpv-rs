use cssparser::{Parser as CssParser, ParserInput as CssParserInput, Token};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};
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

pub fn render_markdown(
    input: &str,
    session_id: Option<Uuid>,
    mermaid_enabled: bool,
) -> RenderedMarkdown {
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
    let mut parser = Parser::new_ext(input, options).into_offset_iter();
    let mut events = Vec::new();
    let mut diagrams = Vec::new();
    let mut has_mermaid = false;
    while let Some((event, range)) = parser.next() {
        let is_mermaid = matches!(&event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
            if info.split(|ch: char| ch.is_whitespace() || ch == '{')
                .next().is_some_and(|lang| lang.eq_ignore_ascii_case("mermaid")));
        has_mermaid |= is_mermaid;
        if is_mermaid && mermaid_enabled {
            events.push(source_anchor(source_line(
                input,
                range.start,
                stripped.line_offset,
            )));
            let mut source = String::new();
            for (event, _) in parser.by_ref() {
                match event {
                    Event::End(TagEnd::CodeBlock) => break,
                    Event::Text(text) => source.push_str(&text),
                    _ => {}
                }
            }
            match crate::mermaid::render_image(&source) {
                Ok(image) => {
                    let placeholder = format!(r#"<span id="mdlive-mermaid-{}"></span>"#, Uuid::new_v4());
                    events.push(Event::Html(placeholder.clone().into()));
                    diagrams.push((placeholder, image));
                }
                Err(error) => events.push(Event::Html(format!(
                    "<div class=\"mermaid-error\"><p>Mermaid: {}</p><pre><code class=\"language-mermaid\">{}</code></pre></div>",
                    escape_html_text(&error.to_string()), escape_html_text(&source)
                ).into())),
            }
        } else {
            events.extend(render_events(
                event,
                range,
                input,
                stripped.line_offset,
                session_id,
            ));
        }
    }
    let mut html = String::new();
    html::push_html(&mut html, events.into_iter());
    let mut html = sanitize_rendered_html(&html);
    // Only server-generated, unpredictable placeholders can bypass HTML sanitization.
    // SVG stays in an image document, isolated from the viewer's DOM and scripts.
    for (placeholder, image) in diagrams {
        html = html.replace(&placeholder, &image);
    }

    RenderedMarkdown {
        html,
        has_mermaid,
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
    let mut input = CssParserInput::new(style);
    let mut parser = CssParser::new(&mut input);
    let mut property_ranges = Vec::new();
    let mut at_declaration_start = true;

    while !parser.is_exhausted() {
        let start = parser.position();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };

        match token {
            Token::WhiteSpace(_) | Token::Comment(_) if at_declaration_start => {}
            Token::Ident(_) if at_declaration_start => {
                property_ranges.push(start.byte_index()..parser.position().byte_index());
                at_declaration_start = false;
            }
            Token::Semicolon => at_declaration_start = true,
            _ => at_declaration_start = false,
        }
    }

    let mut normalized = style.to_owned();
    for range in property_ranges.into_iter().rev() {
        normalized.replace_range(range.clone(), &style[range].to_ascii_lowercase());
    }
    normalized
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

    fn render_markdown(input: &str, session_id: Option<Uuid>) -> RenderedMarkdown {
        super::render_markdown(input, session_id, true)
    }

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

    fn svg_images(html: &str) -> Vec<String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        html.split("data:image/svg+xml;base64,")
            .skip(1)
            .map(|part| {
                String::from_utf8(STANDARD.decode(part.split('"').next().unwrap()).unwrap())
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn renders_mermaid_families_as_self_contained_images() {
        for source in [
            "flowchart TD\nA[開始]-->B[終了]",
            "sequenceDiagram\nAlice->>Bob: Hello",
            "classDiagram\nAnimal <|-- Duck",
            "stateDiagram-v2\n[*] --> Active",
            "erDiagram\nCUSTOMER ||--o{ ORDER : places",
        ] {
            let rendered = render_markdown(&format!("```mermaid\n{source}\n```"), None);
            let images = svg_images(&rendered.html);
            assert_eq!(images.len(), 1, "{source}: {}", rendered.html);
            assert!(images[0].contains("<svg"));
            assert!(images[0].contains("viewBox"));
            let svg = roxmltree::Document::parse(&images[0]).unwrap();
            let view_box: Vec<f64> = svg
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            assert!(rendered.html.contains(&format!(
                r#"width="{}" height="{}""#,
                view_box[2].ceil(),
                view_box[3].ceil()
            )));
            assert!(!images[0].contains("<foreignObject"));
            if source.contains("開始") {
                assert!(images[0].contains("開始"));
            }
        }
    }

    #[test]
    fn renders_multiple_diagrams_with_attributes_and_source_anchors() {
        let rendered = render_markdown(
            "---\ntitle: A\n---\n~~~ MeRmAiD {#chart}\ngraph TD\nA-->B\n~~~\n\n```mermaid\ngraph LR\nC-->D\n```",
            None,
        );
        assert_eq!(svg_images(&rendered.html).len(), 2);
        assert!(rendered.html.contains(r#"data-source-line="4""#));
        assert!(rendered.html.contains(r#"data-source-line="9""#));
        assert!(!rendered.html.contains("mdlive-mermaid-"));
    }

    #[test]
    fn disabled_mermaid_preserves_code_and_detection() {
        let rendered = super::render_markdown("```mermaid\ngraph TD\nA-->B\n```", None, false);
        assert!(rendered.has_mermaid);
        assert!(rendered.html.contains(r#"class="language-mermaid""#));
        assert!(svg_images(&rendered.html).is_empty());
    }

    #[test]
    fn does_not_detect_mermaid_inside_other_code_or_plain_text() {
        for source in [
            "````text\n```mermaid\ngraph TD\nA-->B\n```\n````",
            "text: ```mermaid",
            "    ```mermaid\n    graph TD",
        ] {
            let rendered = render_markdown(source, None);
            assert!(!rendered.has_mermaid, "{source}");
            assert!(svg_images(&rendered.html).is_empty());
        }
    }

    #[test]
    fn malformed_diagram_preserves_source_and_other_blocks() {
        let rendered = render_markdown(
            "```mermaid\nnot a diagram <script>alert(1)</script>\n```\n\n# Still here\n\n```mermaid\ngraph TD\nA-->B\n```",
            None,
        );
        assert!(rendered.html.contains("mermaid-error"));
        assert!(rendered.html.contains("not a diagram &lt;script&gt;"));
        assert!(!rendered.html.contains("<script>"));
        assert!(rendered.html.contains("<h1>Still here</h1>"));
        assert_eq!(svg_images(&rendered.html).len(), 1);
    }

    #[test]
    fn image_embedding_does_not_weaken_raw_html_sanitization() {
        let rendered = render_markdown(
            r#"<img src="data:image/svg+xml;base64,PHN2Zz4=" onerror="alert(1)">
<svg onload="alert(1)"></svg>

```mermaid
%%{init: {"htmlLabels": true, "securityLevel": "loose"}}%%
flowchart TD
A["<img src=x onerror=alert(1)>"]
```"#,
            None,
        );
        assert!(!rendered.html.contains("<svg"));
        assert!(!rendered.html.contains("onerror="));
        assert_eq!(svg_images(&rendered.html).len(), 1);
        assert!(!svg_images(&rendered.html)[0].contains("<foreignObject"));
    }

    #[test]
    fn oversized_diagram_reports_resource_limit() {
        let limit = merman::svg::RenderResourcePolicy::default()
            .value(merman::svg::ResourceLimitId::MaxSourceBytes)
            .unwrap();
        let source = format!("```mermaid\ngraph TD\n%% {}\n```", "x".repeat(limit));
        let rendered = render_markdown(&source, None);
        assert!(rendered.html.contains("mermaid-error"));
        assert!(rendered.html.contains("max_source_bytes"));
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
    fn style_normalization_preserves_delimiters_inside_values() {
        let style = r#"FONT-FAMILY:"ACME;Variant:UPPER"; COLOR:rgb(1; 2; 3)"#;

        assert_eq!(
            normalize_style_property_names(style),
            r#"font-family:"ACME;Variant:UPPER"; color:rgb(1; 2; 3)"#
        );
    }

    #[test]
    fn sanitizer_preserves_delimiters_inside_style_values() {
        let html = sanitize_rendered_html(
            r#"<section style='FONT-FAMILY:"ACME;Variant:UPPER"'>styled</section>"#,
        );

        assert!(html.contains(r#"font-family:&quot;ACME;Variant:UPPER&quot;"#));
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
        assert!(rendered.html.contains("data:image/svg+xml;base64,"));
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
