//! Markdown → HTML body conversion.
//!
//! The pipeline is: parse (pulldown-cmark, GFM extensions) → post-process the
//! event stream (front matter, heading anchors) → render to HTML → sanitize.

use std::collections::HashMap;

use pulldown_cmark::{
    html, CodeBlockKind, CowStr, Event, HeadingLevel, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};

use crate::{highlight, sanitize};

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub smart_punctuation: bool,
    pub unsafe_html: bool,
    pub highlight: bool,
}

#[derive(Debug, Clone)]
pub struct Rendered {
    /// Sanitized HTML for the `<body>` content.
    pub body: String,
    /// Best-effort document title (front matter, then first heading).
    pub title: Option<String>,
    /// True when at least one code block was highlighted (so the highlight CSS
    /// is needed).
    pub uses_highlight: bool,
}

pub fn render(source: &str, opts: &RenderOptions) -> Rendered {
    let mut parser_opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_GFM;
    if opts.smart_punctuation {
        parser_opts |= Options::ENABLE_SMART_PUNCTUATION;
    }

    let events: Vec<Event> = Parser::new_ext(source, parser_opts).collect();

    let (events, front_matter_title) = strip_front_matter(events);
    let (events, heading_title) = assign_heading_ids(events);
    let (events, uses_highlight) = if opts.highlight {
        highlight_code_blocks(events)
    } else {
        (events, false)
    };

    let mut html_out = String::with_capacity(source.len() * 2);
    html::push_html(&mut html_out, events.into_iter());

    let body = if opts.unsafe_html {
        html_out
    } else {
        sanitize::clean(&html_out)
    };

    Rendered {
        body,
        title: front_matter_title.or(heading_title),
        uses_highlight,
    }
}

/// Removes a leading YAML front matter block from the stream and returns the
/// `title:` value when there is one.
fn strip_front_matter(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, Option<String>) {
    let mut out = Vec::with_capacity(events.len());
    let mut title = None;
    let mut in_meta = false;

    for event in events {
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => in_meta = true,
            Event::End(TagEnd::MetadataBlock(_)) => in_meta = false,
            Event::Text(text) if in_meta => {
                if title.is_none() {
                    title = yaml_title(&text);
                }
            }
            other if !in_meta => out.push(other),
            _ => {}
        }
    }
    (out, title)
}

/// Tiny `title:` extractor — enough for front matter without a YAML parser.
fn yaml_title(block: &str) -> Option<String> {
    block.lines().find_map(|line| {
        let value = line.strip_prefix("title:")?.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// Gives every heading a GitHub-style anchor id (unless one was written with
/// `{#custom-id}`) and returns the text of the first top-level heading.
fn assign_heading_ids(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, Option<String>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut first_title: Option<String> = None;
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    let mut open_heading: Option<usize> = None;
    let mut text = String::new();

    for event in events {
        match &event {
            Event::Start(Tag::Heading { .. }) => {
                open_heading = Some(out.len());
                text.clear();
            }
            Event::Text(t) | Event::Code(t) if open_heading.is_some() => text.push_str(t),
            Event::End(TagEnd::Heading(level)) => {
                if let Some(start) = open_heading.take() {
                    if *level == HeadingLevel::H1 && first_title.is_none() && !text.trim().is_empty() {
                        first_title = Some(text.trim().to_string());
                    }
                    if let Event::Start(Tag::Heading { id, .. }) = &mut out[start] {
                        if id.is_none() {
                            *id = Some(CowStr::from(unique_slug(&text, &mut seen)));
                        }
                    }
                }
            }
            _ => {}
        }
        out.push(event);
    }
    (out, first_title)
}

/// Replaces fenced code blocks that have a known language with pre-highlighted
/// HTML. Blocks without a language, or with an unknown one, stay untouched.
fn highlight_code_blocks(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, bool) {
    let mut out = Vec::with_capacity(events.len());
    let mut used = false;
    let mut iter = events.into_iter();

    while let Some(event) = iter.next() {
        let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = &event else {
            out.push(event);
            continue;
        };
        let info = info.to_string();

        let mut body = Vec::new();
        let mut code = String::new();
        for inner in iter.by_ref() {
            match inner {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(t) => {
                    code.push_str(&t);
                    body.push(Event::Text(t));
                }
                other => body.push(other),
            }
        }

        match highlight::highlight(&info, &code) {
            Some(inner) => {
                used = true;
                let lang = crate::page::escape(highlight::language(&info));
                out.push(Event::Html(CowStr::from(format!(
                    "<pre><code class=\"language-{lang}\">{inner}</code></pre>\n"
                ))));
            }
            None => {
                out.push(event);
                out.extend(body);
                out.push(Event::End(TagEnd::CodeBlock));
            }
        }
    }
    (out, used)
}

fn unique_slug(text: &str, seen: &mut HashMap<String, usize>) -> String {
    let base = slugify(text);
    let count = seen.entry(base.clone()).or_insert(0);
    let slug = if *count == 0 {
        base
    } else {
        format!("{base}-{count}")
    };
    *count += 1;
    slug
}

/// GitHub-compatible slug: lowercase, spaces → `-`, punctuation dropped, other
/// scripts (e.g. CJK) kept as they are.
fn slugify(text: &str) -> String {
    let slug: String = text
        .trim()
        .chars()
        .flat_map(|c| -> Vec<char> {
            if c.is_alphanumeric() {
                c.to_lowercase().collect()
            } else if c == '-' || c == '_' {
                vec![c]
            } else if c.is_whitespace() {
                vec!['-']
            } else {
                vec![]
            }
        })
        .collect();
    if slug.is_empty() { "section".into() } else { slug }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(md: &str) -> String {
        render(md, &RenderOptions::default()).body
    }

    fn highlighted(md: &str) -> Rendered {
        render(md, &RenderOptions { highlight: true, ..Default::default() })
    }

    #[test]
    fn renders_common_markdown() {
        let html = body("# Hi\n\n**bold** *em* ~~del~~ `code`\n\n- a\n- b\n");
        assert!(html.contains("<h1 id=\"hi\">Hi</h1>"), "{html}");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>em</em>"));
        assert!(html.contains("<del>del</del>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<li>a</li>"));
    }

    #[test]
    fn renders_gfm_table_and_tasklist() {
        let html = body("| a | b |\n|:-|-:|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n");
        assert!(html.contains("<table>"), "{html}");
        assert!(html.contains("text-align: left"), "{html}");
        assert!(html.contains("checked"), "{html}");
        assert!(html.contains("type=\"checkbox\""), "{html}");
    }

    #[test]
    fn renders_footnotes() {
        let html = body("text[^1]\n\n[^1]: the note\n");
        assert!(html.contains("footnote"), "{html}");
        assert!(html.contains("the note"));
    }

    #[test]
    fn heading_ids_are_unique_and_unicode_friendly() {
        let html = body("# 你好 World\n\n## Same\n\n## Same\n");
        assert!(html.contains("id=\"你好-world\""), "{html}");
        assert!(html.contains("id=\"same\""));
        assert!(html.contains("id=\"same-1\""));
    }

    #[test]
    fn explicit_heading_id_is_kept() {
        assert!(body("# Title {#custom}\n").contains("id=\"custom\""));
    }

    #[test]
    fn front_matter_is_hidden_and_supplies_title() {
        let r = render("---\ntitle: \"My Doc\"\nauthor: x\n---\n\n# Heading\n", &RenderOptions::default());
        assert_eq!(r.title.as_deref(), Some("My Doc"));
        assert!(!r.body.contains("author"));
    }

    #[test]
    fn title_falls_back_to_first_h1() {
        let r = render("intro\n\n# First\n\n# Second\n", &RenderOptions::default());
        assert_eq!(r.title.as_deref(), Some("First"));
    }

    #[test]
    fn script_is_removed_unless_unsafe() {
        let md = "hi <script>alert(1)</script> <img src=x onerror=alert(1)>\n\n[x](javascript:alert(1))\n";
        let safe = body(md);
        assert!(!safe.contains("<script"), "{safe}");
        assert!(!safe.contains("onerror"), "{safe}");
        assert!(!safe.contains("javascript:"), "{safe}");

        let raw = render(md, &RenderOptions { unsafe_html: true, ..Default::default() }).body;
        assert!(raw.contains("<script>"));
    }

    #[test]
    fn fenced_code_is_highlighted_when_enabled() {
        let r = highlighted("```rust\nfn main() {}\n```\n");
        assert!(r.uses_highlight);
        assert!(r.body.contains("language-rust"), "{}", r.body);
        assert!(r.body.contains("<span class=\"hl-"), "{}", r.body);
    }

    #[test]
    fn highlighting_is_off_by_default_and_skips_unknown_languages() {
        let md = "```rust\nfn main() {}\n```\n";
        assert!(!render(md, &RenderOptions::default()).uses_highlight);
        let r = highlighted("```nope\n<b>x</b>\n```\n\n```\nplain\n```\n");
        assert!(!r.uses_highlight);
        assert!(r.body.contains("&lt;b&gt;x&lt;/b&gt;"), "{}", r.body);
        assert!(r.body.contains("plain"));
    }
}
