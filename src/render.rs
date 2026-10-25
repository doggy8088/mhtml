//! Markdown → HTML body conversion.
//!
//! The pipeline is: parse (pulldown-cmark, GFM extensions) → post-process the
//! event stream (front matter, heading anchors) → render to HTML → sanitize.

use std::collections::HashMap;

use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, MetadataBlockKind, Options, Parser, Tag, TagEnd,
    html,
};

use crate::{highlight, page, sanitize};

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub smart_punctuation: bool,
    pub unsafe_html: bool,
    pub highlight: bool,
    pub math: bool,
    /// Render ```mermaid fences with the embedded Ondine renderer.
    pub mermaid: bool,
    /// Prepend a table of contents with this heading text.
    pub toc_title: Option<String>,
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
    /// True when at least one Mermaid diagram is present (so the renderer is
    /// embedded in the page).
    pub uses_mermaid: bool,
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
    if opts.math {
        parser_opts |= Options::ENABLE_MATH;
    }
    if opts.smart_punctuation {
        parser_opts |= Options::ENABLE_SMART_PUNCTUATION;
    }

    let events: Vec<Event> = Parser::new_ext(source, parser_opts).collect();

    let (events, front_matter_title) = strip_front_matter(events);
    let (events, headings) = assign_heading_ids(events);
    let events = if opts.math {
        render_math(events)
    } else {
        events
    };
    let (events, uses_mermaid) = if opts.mermaid {
        render_mermaid(events)
    } else {
        (events, false)
    };
    let (events, uses_highlight) = if opts.highlight {
        highlight_code_blocks(events)
    } else {
        (events, false)
    };

    let mut html_out = String::with_capacity(source.len() * 2);
    html::push_html(&mut html_out, events.into_iter());

    let mut body = if opts.unsafe_html {
        html_out
    } else {
        sanitize::clean(&html_out)
    };
    if let Some(toc_title) = &opts.toc_title {
        // Built from escaped text only, so it is added after sanitizing. It goes
        // right after a leading page title (H1), otherwise at the very top.
        let at = match headings.first() {
            Some(h) if h.level == HeadingLevel::H1 => {
                body.find("</h1>").map_or(0, |i| i + "</h1>".len() + 1)
            }
            _ => 0,
        };
        body.insert_str(at.min(body.len()), &build_toc(&headings, toc_title));
    }
    let heading_title = headings
        .iter()
        .find(|h| h.level == HeadingLevel::H1)
        .map(|h| h.text.clone());

    Rendered {
        body,
        title: front_matter_title.or(heading_title),
        uses_highlight,
        uses_mermaid,
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

struct Heading {
    level: HeadingLevel,
    id: String,
    text: String,
}

/// Gives every heading a GitHub-style anchor id (unless one was written with
/// `{#custom-id}`) and returns the headings in document order.
fn assign_heading_ids(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, Vec<Heading>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut headings = Vec::new();
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
                if let Some(start) = open_heading.take()
                    && let Event::Start(Tag::Heading { id, .. }) = &mut out[start]
                {
                    if id.is_none() {
                        *id = Some(CowStr::from(unique_slug(&text, &mut seen)));
                    }
                    headings.push(Heading {
                        level: *level,
                        id: id.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                        text: text.trim().to_string(),
                    });
                }
            }
            _ => {}
        }
        out.push(event);
    }
    (out, headings)
}

/// Nested `<nav class="toc">` list. A lone H1 is the page title, so it is left
/// out; the list goes down to H4.
fn build_toc(headings: &[Heading], title: &str) -> String {
    let h1_count = headings
        .iter()
        .filter(|h| h.level == HeadingLevel::H1)
        .count();
    let min_level = if h1_count == 1 { 2 } else { 1 };
    let items: Vec<(usize, &Heading)> = headings
        .iter()
        .map(|h| (h.level as usize, h))
        .filter(|(l, h)| *l >= min_level && *l <= 4 && !h.text.is_empty())
        .collect();
    if items.is_empty() {
        return String::new();
    }

    let mut html = format!(
        "<nav class=\"toc\">\n<strong>{}</strong>\n",
        page::escape(title)
    );
    let mut stack: Vec<usize> = Vec::new();
    for (level, h) in items {
        while stack.last().is_some_and(|&top| top > level) {
            stack.pop();
            html.push_str("</li>\n</ul>\n");
        }
        match stack.last() {
            Some(&top) if top == level => html.push_str("</li>\n"),
            _ => {
                html.push_str("<ul>\n");
                stack.push(level);
            }
        }
        html.push_str(&format!(
            "<li><a href=\"#{}\">{}</a>",
            page::escape(&h.id),
            page::escape(&h.text)
        ));
    }
    for _ in &stack {
        html.push_str("</li>\n</ul>\n");
    }
    html.push_str("</nav>\n");
    html
}

/// Converts `$…$`, `$$…$$` and ```` ```math ```` blocks to MathML, which every
/// current browser renders natively, so no JavaScript or font files are needed.
fn render_math(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    use latex2mathml::{DisplayStyle, latex_to_mathml};

    let convert = |latex: &str, style: DisplayStyle| -> Event<'static> {
        let html = latex_to_mathml(latex, style).unwrap_or_else(|err| {
            format!(
                "<code class=\"math-error\" title=\"{}\">{}</code>",
                page::escape(&err.to_string()),
                page::escape(latex)
            )
        });
        Event::Html(CowStr::from(html))
    };

    let mut out = Vec::with_capacity(events.len());
    let mut iter = events.into_iter();
    while let Some(event) = iter.next() {
        match event {
            Event::InlineMath(latex) => out.push(convert(&latex, DisplayStyle::Inline)),
            Event::DisplayMath(latex) => {
                out.push(convert(&latex, DisplayStyle::Block));
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info)))
                if info.trim() == "math" =>
            {
                let mut latex = String::new();
                for inner in iter.by_ref() {
                    match inner {
                        Event::End(TagEnd::CodeBlock) => break,
                        Event::Text(t) => latex.push_str(&t),
                        _ => {}
                    }
                }
                out.push(convert(&latex, DisplayStyle::Block));
            }
            other => out.push(other),
        }
    }
    out
}

/// Replaces ```` ```mermaid ```` fences with a wrapper that the embedded
/// renderer turns into an SVG in the browser. The source is escaped, so the
/// page stays safe even with `--unsafe-html`, and it stays visible until (or
/// instead of) rendering.
fn render_mermaid(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, bool) {
    let mut out = Vec::with_capacity(events.len());
    let mut used = false;
    let mut iter = events.into_iter();

    while let Some(event) = iter.next() {
        let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = &event else {
            out.push(event);
            continue;
        };
        if info.split_whitespace().next() != Some("mermaid") {
            out.push(event);
            continue;
        }

        let mut source = String::new();
        for inner in iter.by_ref() {
            match inner {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(t) => source.push_str(&t),
                _ => {}
            }
        }
        used = true;
        out.push(Event::Html(CowStr::from(format!(
            "<div class=\"mermaid-diagram\"><pre><code class=\"language-mermaid\">{}</code></pre></div>\n",
            page::escape(&source)
        ))));
    }
    (out, used)
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
    if slug.is_empty() {
        "section".into()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(md: &str) -> String {
        render(md, &RenderOptions::default()).body
    }

    fn highlighted(md: &str) -> Rendered {
        render(
            md,
            &RenderOptions {
                highlight: true,
                ..Default::default()
            },
        )
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
        let r = render(
            "---\ntitle: \"My Doc\"\nauthor: x\n---\n\n# Heading\n",
            &RenderOptions::default(),
        );
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

        let raw = render(
            md,
            &RenderOptions {
                unsafe_html: true,
                ..Default::default()
            },
        )
        .body;
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

    fn with(f: impl FnOnce(&mut RenderOptions)) -> RenderOptions {
        let mut o = RenderOptions::default();
        f(&mut o);
        o
    }

    #[test]
    fn math_is_rendered_as_mathml() {
        let o = with(|o| o.math = true);
        let inline = render("Euler: $e^{i\\pi}+1=0$ ok", &o).body;
        assert!(inline.contains("<math"), "{inline}");
        assert!(inline.contains("<msup>"), "{inline}");
        assert!(!inline.contains("display=\"block\""), "{inline}");

        let block = render("$$\\frac{a}{b}$$\n\n```math\n\\sqrt{x}\n```\n", &o).body;
        assert!(block.contains("<mfrac>"), "{block}");
        assert!(block.contains("<msqrt>"), "{block}");
        assert!(block.contains("display=\"block\""), "{block}");
    }

    #[test]
    fn math_can_be_disabled_and_currency_is_not_math() {
        let off = render("$a$ and\n\n```math\nx\n```\n", &RenderOptions::default()).body;
        assert!(!off.contains("<math"), "{off}");
        let money = render("It costs $5 and $6.", &with(|o| o.math = true)).body;
        assert!(!money.contains("<math"), "{money}");
    }

    #[test]
    fn mermaid_fences_become_diagram_wrappers() {
        let o = with(|o| o.mermaid = true);
        let r = render(
            "```mermaid title=\"Flow\"\nflowchart LR\n  A --> B\n```\n",
            &o,
        );
        assert!(r.uses_mermaid);
        assert!(
            r.body.contains("<div class=\"mermaid-diagram\">"),
            "{}",
            r.body
        );
        assert!(r.body.contains("language-mermaid"), "{}", r.body);
        assert!(r.body.contains("A --&gt; B"), "{}", r.body);
    }

    #[test]
    fn mermaid_is_off_by_default_and_keeps_the_code_block() {
        let md = "```mermaid\nflowchart LR\n  A --> B\n```\n";
        let r = render(md, &RenderOptions::default());
        assert!(!r.uses_mermaid);
        assert!(!r.body.contains("mermaid-diagram"), "{}", r.body);
        assert!(r.body.contains("language-mermaid"), "{}", r.body);
        assert!(r.body.contains("A --&gt; B"), "{}", r.body);
    }

    #[test]
    fn mermaid_source_is_escaped() {
        let o = with(|o| o.mermaid = true);
        let r = render(
            "```mermaid\nA[<img src=x onerror=alert(1)>] --> B\n```\n",
            &o,
        );
        assert!(!r.body.contains("<img"), "{}", r.body);
        assert!(r.body.contains("&lt;img"), "{}", r.body);
    }

    #[test]
    fn toc_lists_headings_without_the_lone_title() {
        let o = with(|o| o.toc_title = Some("目錄".into()));
        let body = render("# Title\n\n## A\n\n### A1\n\n## B & C\n", &o).body;
        let (title, rest) = body.split_once("</h1>").unwrap();
        assert!(
            title.contains("Title") && rest.trim_start().starts_with("<nav class=\"toc\">"),
            "{body}"
        );
        assert!(body.contains("<strong>目錄</strong>"));
        assert!(body.contains("<a href=\"#a\">A</a>"), "{body}");
        assert!(body.contains("<a href=\"#a1\">A1</a>"), "{body}");
        assert!(body.contains("B &amp; C"), "{body}");
        assert!(!body.contains("<a href=\"#title\">"), "{body}");
        assert_eq!(body.matches("<ul>").count(), body.matches("</ul>").count());
    }

    #[test]
    fn no_toc_by_default_or_without_headings() {
        assert!(!body("# T\n\n## A\n").contains("class=\"toc\""));
        let o = with(|o| o.toc_title = Some("x".into()));
        assert!(!render("just text\n", &o).body.contains("toc"));
        // Without a leading H1 the TOC goes first.
        assert!(
            render("intro\n\n## A\n\n## B\n", &o)
                .body
                .starts_with("<nav class=\"toc\">")
        );
    }
}
