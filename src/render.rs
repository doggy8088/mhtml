//! Markdown → HTML body conversion.
//!
//! The pipeline is: parse (pulldown-cmark, GFM extensions) → post-process the
//! event stream (front matter, heading anchors) → render to HTML → sanitize.

use std::collections::HashMap;

use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, MetadataBlockKind, Options, Parser, Tag, TagEnd,
    html,
};

use crate::enhance::{self, Profile};
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
    /// Adaptive presentation: classify lists, tables and headings, and build a
    /// side outline for long documents.
    pub enhance: bool,
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
    /// Document profile from the adaptive pass (empty when it is off).
    pub profile: Profile,
    /// Sticky side outline (`<nav class="toc toc-side">`) for long documents.
    pub outline: Option<String>,
    /// The document has its own H1 (otherwise the page may show its title).
    pub has_h1: bool,
    /// Emphasis markers that CommonMark/GitHub leave literal.
    pub warnings: Vec<Warning>,
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

    let normalized;
    let source = if opts.math {
        normalized = normalize_inline_math(source);
        normalized.as_str()
    } else {
        source
    };

    let mut events = Vec::new();
    let mut literal_stars = Vec::new();
    let mut paired_stars = Vec::new();
    let mut prose = true;
    for (event, range) in Parser::new_ext(source, parser_opts).into_offset_iter() {
        match &event {
            Event::Start(Tag::CodeBlock(_)) | Event::Start(Tag::MetadataBlock(_)) => prose = false,
            Event::End(TagEnd::CodeBlock) | Event::End(TagEnd::MetadataBlock(_)) => prose = true,
            Event::Text(_) if prose => collect_literal_stars(source, range, &mut literal_stars),
            Event::Start(Tag::Strong) | Event::Start(Tag::Emphasis) => {
                collect_span_stars(source, range, &mut paired_stars);
            }
            _ => {}
        }
        events.push(event);
    }
    let mut all_stars = literal_stars.clone();
    all_stars.extend(paired_stars);
    let runs = star_runs(source, &all_stars);
    // The warnings describe what CommonMark (and GitHub) does, so they come
    // from the unpatched parse — a spot that mhtml renders bold still warns.
    let warnings = diagnose_literal_emphasis(source, &star_runs(source, &literal_stars));
    let patched;
    let events = if runs.iter().any(|r| r.open_fix || r.close_fix) {
        // Some delimiter runs are only unflanking because CommonMark chokes on
        // a CJK neighbor (or pair with the wrong partner); re-parse with the
        // flanking characters nudged (see `FLANK_FIX`).
        patched = insert_flank_fixes(source, &runs);
        drop_flank_fixes(Parser::new_ext(&patched, parser_opts).collect())
    } else {
        drop_flank_fixes(events)
    };

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
    let (events, profile) = if opts.enhance {
        enhance::enhance(events)
    } else {
        (events, Profile::default())
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
                let after_h1 = body.find("</h1>").map_or(0, |i| i + "</h1>".len() + 1);
                // Keep the reading-time line under the title, above the TOC.
                match body[after_h1.min(body.len())..].strip_prefix("<p class=\"doc-meta\">") {
                    Some(_) => body[after_h1..]
                        .find("</p>")
                        .map_or(after_h1, |i| after_h1 + i + "</p>".len() + 1),
                    None => after_h1,
                }
            }
            _ => 0,
        };
        body.insert_str(at.min(body.len()), &build_toc(&headings, toc_title));
    }
    let heading_title = headings
        .iter()
        .find(|h| h.level == HeadingLevel::H1)
        .map(|h| h.text.clone());
    let outline = profile.outline.then(|| {
        let title = opts.toc_title.clone().unwrap_or_else(|| {
            if profile.classes.contains(&"is-cjk") {
                "目錄".into()
            } else {
                "Contents".into()
            }
        });
        build_outline(&headings, &title)
    });

    Rendered {
        body,
        title: front_matter_title.or(heading_title.clone()),
        uses_highlight,
        uses_mermaid,
        profile,
        outline,
        has_h1: heading_title.is_some(),
        warnings,
    }
}

/// H2/H3 outline for the sticky sidebar. Like the TOC it is built from escaped
/// text only, so it is added after sanitizing.
fn build_outline(headings: &[Heading], title: &str) -> String {
    let mut html = format!(
        "<nav class=\"toc toc-side\" aria-label=\"{0}\">\n<strong>{0}</strong>\n<ul>\n",
        page::escape(title)
    );
    let mut open_sub = false;
    let mut open_item = false;
    for h in headings.iter().filter(|h| !h.text.is_empty()) {
        let link = format!(
            "<a href=\"#{}\">{}</a>",
            page::escape(&h.id),
            page::escape(&h.text)
        );
        match h.level {
            HeadingLevel::H2 => {
                if open_sub {
                    html.push_str("</ul>\n");
                    open_sub = false;
                }
                if open_item {
                    html.push_str("</li>\n");
                }
                html.push_str(&format!("<li>{link}"));
                open_item = true;
            }
            HeadingLevel::H3 => {
                if !open_sub {
                    html.push_str(if open_item {
                        "\n<ul>\n"
                    } else {
                        "<li>\n<ul>\n"
                    });
                    open_sub = true;
                    open_item = true;
                }
                html.push_str(&format!("<li>{link}</li>\n"));
            }
            _ => {}
        }
    }
    if open_sub {
        html.push_str("</ul>\n");
    }
    if open_item {
        html.push_str("</li>\n");
    }
    html.push_str("</ul>\n</nav>\n");
    html
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
            Event::Text(text) if in_meta && title.is_none() => title = yaml_title(&text),
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

/// CommonMark's flanking rules treat CJK and fullwidth punctuation like ASCII
/// punctuation, so `**標籤：**內容` never pairs and its asterisks show up as
/// literal text (GitHub does the same). The markdown-cjk-friendly amendments
/// (<https://github.com/tats-u/markdown-cjk-friendly>) relax those rules next
/// to CJK characters; instead of patching the parser, mhtml swaps the flanking
/// neighbor of every delimiter run where the two rule sets disagree for
/// `FLANK_FIX` and parses again. The empty comment makes the neighbor ASCII
/// punctuation — exactly the relaxed verdict — and is filtered out afterwards.
const FLANK_FIX: &str = "<!---->";

/// A maximal `*` delimiter run in the source, paired up or not.
struct StarRun {
    start: usize,
    end: usize,
    /// The CJK-friendly rules make it left-flanking, CommonMark does not;
    /// `FLANK_FIX` goes before the run to fix that.
    open_fix: bool,
    /// The CJK-friendly rules make it right-flanking, CommonMark does not;
    /// `FLANK_FIX` goes after the run to fix that.
    close_fix: bool,
    /// Could be an emphasis marker at all (left- or right-flanking). `a * b`
    /// cannot, so it is not an unpaired emphasis attempt.
    shaped: bool,
}

/// Collects the byte offsets of the `*` characters that parsed as literal text
/// inside `range`. Code spans never become Text events and `\*` escapes are
/// skipped, so only real delimiter runs remain. Paired delimiters are consumed
/// by `Start(Strong)`/`Start(Emphasis)` and never show up here.
fn collect_literal_stars(source: &str, range: std::ops::Range<usize>, out: &mut Vec<usize>) {
    let bytes = source.as_bytes();
    let text = &source[range.clone()];
    let mut i = 0;
    while i < text.len() {
        if text.as_bytes()[i] == b'*' {
            let start = i;
            while i < text.len() && text.as_bytes()[i] == b'*' {
                i += 1;
            }
            let abs = range.start + start;
            if abs == 0 || bytes[abs - 1] != b'\\' {
                out.extend(abs..range.start + i);
            }
        } else {
            // UTF-8 continuation bytes are never `*`, so a byte walk is safe.
            i += 1;
        }
    }
}

/// The delimiter runs that paired up sit at the edges of their span
/// (`**…**`). Escaped stars are text, so they stop the scan.
fn collect_span_stars(source: &str, range: std::ops::Range<usize>, out: &mut Vec<usize>) {
    let bytes = source.as_bytes();
    let mut start = range.start;
    while start < range.end && bytes[start] == b'*' && (start == 0 || bytes[start - 1] != b'\\') {
        out.push(start);
        start += 1;
    }
    let mut end = range.end;
    while end > range.start && bytes[end - 1] == b'*' {
        if end >= 2 && bytes[end - 2] == b'\\' {
            break;
        }
        out.push(end - 1);
        end -= 1;
    }
}

/// Groups the `*` offsets into delimiter runs and works out where the
/// CJK-friendly rules disagree with CommonMark. The neighbors alone cannot
/// tell a working opener (`。**已`) from a broken closer (`：**2`) — the same
/// shape on both sides — so the verdict comes from comparing the two rule
/// sets on the run itself: a fix is only placed where the parse would change.
fn star_runs(source: &str, stars: &[usize]) -> Vec<StarRun> {
    let mut stars = stars.to_vec();
    stars.sort_unstable();
    stars.dedup();

    let mut runs: Vec<StarRun> = Vec::new();
    for &pos in &stars {
        match runs.last_mut() {
            Some(last) if last.end == pos => last.end = pos + 1,
            _ => runs.push(StarRun {
                start: pos,
                end: pos + 1,
                open_fix: false,
                close_fix: false,
                shaped: false,
            }),
        }
    }

    for run in &mut runs {
        let before = source[..run.start].chars().next_back();
        let after = source[run.end..].chars().next();
        let p_ws = before.is_none_or(char::is_whitespace);
        let n_ws = after.is_none_or(char::is_whitespace);
        let p_punct = before.is_some_and(is_punct);
        let n_punct = after.is_some_and(is_punct);
        run.shaped = (!n_ws && (!n_punct || p_ws || p_punct)) // left-flanking
            || (!p_ws && (!p_punct || n_ws || n_punct)); // right-flanking
        // Where the amendments differ from CommonMark: (2a) a CJK punctuation
        // neighbor no longer counts as punctuation, or (2b) a non-CJK
        // punctuation neighbor still flanks around a CJK character. Both fixes
        // only fire next to CJK/fullwidth characters — plain `**Note:**text`
        // keeps its literal asterisks — and never at a line edge (both
        // neighbors are non-whitespace), so `FLANK_FIX` cannot start an HTML
        // block.
        run.open_fix = n_punct
            && !p_ws
            && !p_punct
            && after.is_some_and(|n| is_cjk_punct(n) || before.is_some_and(is_cjk));
        run.close_fix = p_punct
            && !n_ws
            && !n_punct
            && before.is_some_and(|p| is_cjk_punct(p) || after.is_some_and(is_cjk));
    }
    runs
}

/// Unicode punctuation — the CommonMark definition, approximated with the
/// blocks that can actually neighbor emphasis in prose.
fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '\u{00A1}' | '\u{00A7}' | '\u{00AB}' | '\u{00B6}' | '\u{00B7}' | '\u{00BB}'
                | '\u{00BF}' | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}'
                | '\u{2E00}'..='\u{2E7F}' | '\u{3001}'..='\u{3003}' | '\u{3008}'..='\u{3011}'
                | '\u{3014}'..='\u{301F}' | '\u{3030}' | '\u{303D}' | '\u{30FB}'
                | '\u{FE10}'..='\u{FE1F}' | '\u{FE30}'..='\u{FE6F}' | '\u{FF01}'..='\u{FF0F}'
                | '\u{FF1A}'..='\u{FF20}' | '\u{FF3B}'..='\u{FF40}' | '\u{FF5B}'..='\u{FF65}'
        )
}

/// A CJK character in the sense of the markdown-cjk-friendly spec: East Asian
/// wide/halfwidth (minus emoji) or Hangul.
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF      // hangul jamo
        | 0x2E80..=0x9FFF    // CJK radicals/symbols/punctuation, kana, bopomofo,
                             // hangul compatibility, strokes, enclosed, compat,
                             // extension A and the unified ideographs
        | 0xA960..=0xA97F    // hangul jamo extended-A
        | 0xAC00..=0xD7FF    // hangul syllables
        | 0xF900..=0xFAFF    // CJK compatibility ideographs
        | 0xFE10..=0xFE1F    // vertical forms
        | 0xFE30..=0xFE6F    // compatibility and small form variants
        | 0xFF00..=0xFF9F    // fullwidth forms and halfwidth katakana
        | 0x20000..=0x3FFFF) // extensions B and later
}

fn is_cjk_punct(c: char) -> bool {
    is_cjk(c) && is_punct(c)
}

/// Inserts `FLANK_FIX` before runs that only need to open and after runs that
/// only need to close; the empty HTML comment makes the new neighbor ASCII
/// punctuation, which CommonMark already flanks around.
fn insert_flank_fixes(source: &str, runs: &[StarRun]) -> String {
    let mut out = String::with_capacity(source.len() + runs.len() * FLANK_FIX.len());
    let mut last = 0;
    for run in runs {
        for (at, fix) in [(run.start, run.open_fix), (run.end, run.close_fix)] {
            if fix {
                out.push_str(&source[last..at]);
                out.push_str(FLANK_FIX);
                last = at;
            }
        }
    }
    out.push_str(&source[last..]);
    out
}

/// Removes the flanking fixes (and any authored empty comment) again, so the
/// output never contains `<!---->`, sanitized or not.
fn drop_flank_fixes(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    events
        .into_iter()
        .filter(|e| !matches!(e, Event::InlineHtml(s) | Event::Html(s) if s.as_ref() == FLANK_FIX))
        .collect()
}

/// An emphasis marker that CommonMark — and therefore GitHub — leaves as
/// literal asterisks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// 1-based source line of the first marker of the unpaired attempt.
    pub line: usize,
    /// The marker text, e.g. `**`.
    pub marker: String,
}

/// Reports the emphasis attempts that stay literal under CommonMark rules,
/// even when `FLANK_FIX` makes mhtml render them bold: GitHub and every other
/// CommonMark renderer show the asterisks. Equal-length neighbours on one line
/// (`**…**`) are one attempt, a lone marker is one by itself; asterisks that
/// can never be emphasis (`a * b * c`) are not attempts and stay silent.
fn diagnose_literal_emphasis(source: &str, runs: &[StarRun]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    let mut i = 0;
    while i < runs.len() {
        let line = line_of(source, runs[i].start);
        let mut j = i;
        while j < runs.len() && line_of(source, runs[j].start) == line {
            j += 1;
        }
        let mut k = i;
        while k < j {
            let len = runs[k].end - runs[k].start;
            let paired = k + 1 < j && runs[k + 1].end - runs[k + 1].start == len;
            let last = if paired { k + 1 } else { k };
            if runs[k..=last].iter().any(|r| r.shaped) {
                warnings.push(Warning {
                    line,
                    marker: source[runs[k].start..runs[k].end].to_string(),
                });
            }
            k = last + 1;
        }
        i = j;
    }
    warnings
}

fn line_of(source: &str, offset: usize) -> usize {
    1 + source[..offset].bytes().filter(|&b| b == b'\n').count()
}

/// Tolerant inline math: pulldown-cmark only recognizes `$...$` when the
/// content does not start or end with whitespace, but people often write
/// `$ L $`. Trim that padding when the content still looks like math, while
/// leaving prices and prose alone. Code, display math and `\$` are untouched.
fn normalize_inline_math(source: &str) -> String {
    if !source.contains('$') {
        return source.to_string();
    }
    let mut out = String::with_capacity(source.len());
    let mut fence: Option<(char, usize)> = None;
    let mut in_display = false;

    for (index, line) in source.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        if let Some((marker, len)) = fence {
            if let Some((ch, run)) = fence_marker(line)
                && ch == marker
                && run >= len
            {
                fence = None;
            }
            out.push_str(line);
        } else if let Some((ch, run)) = fence_marker(line) {
            fence = Some((ch, run));
            out.push_str(line);
        } else if in_display {
            if line.trim_end().ends_with("$$") {
                in_display = false;
            }
            out.push_str(line);
        } else if let Some(rest) = display_math_start(line) {
            if !rest.trim_end().ends_with("$$") {
                in_display = true;
            }
            out.push_str(line);
        } else if line.starts_with('\t') || line.starts_with("    ") {
            out.push_str(line);
        } else {
            out.push_str(&normalize_math_line(line));
        }
    }
    out
}

/// Up to three spaces or tabs, then three or more backticks or tildes.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let rest = line.trim_start_matches([' ', '\t']);
    if line.len() - rest.len() > 3 {
        return None;
    }
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let len = rest.chars().take_while(|&c| c == marker).count();
    (len >= 3).then_some((marker, len))
}

/// Text after a line-leading `$$`, when the indentation allows math.
fn display_math_start(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches([' ', '\t']);
    if line.len() - rest.len() > 3 {
        return None;
    }
    rest.strip_prefix("$$")
}

/// Normalizes `$ ... $` pairs on one line, skipping code spans and escapes.
fn normalize_math_line(line: &str) -> String {
    if !line.contains('$') {
        return line.to_string();
    }

    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let ch = rest.chars().next().unwrap();
        match ch {
            '\\' => {
                let mut end = i + 1;
                if let Some(next) = line[end..].chars().next() {
                    end += next.len_utf8();
                }
                out.push_str(&line[i..end]);
                i = end;
            }
            '`' => {
                let run = rest.bytes().take_while(|&b| b == b'`').count();
                let end = code_span_end(line, i + run, run).unwrap_or(i + run);
                out.push_str(&line[i..end]);
                i = end;
            }
            '<' => {
                let end = angle_span_end(line, i).unwrap_or(i + 1);
                out.push_str(&line[i..end]);
                i = end;
            }
            '$' => match closing_dollar(line, i + 1) {
                // `$3.69$148` and `$2.44 > glm $1.43`: a `$` glued to a digit
                // opens the next amount, so the pair is prices, not math.
                Some(close) if line[close + 1..].starts_with(|c: char| c.is_ascii_digit()) => {
                    out.push_str("\\$");
                    i += 1;
                }
                Some(close) => {
                    out.push_str(&normalize_dollar_pair(&line[i + 1..close]));
                    i = close + 1;
                }
                None => {
                    out.push('$');
                    i += 1;
                }
            },
            _ => {
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// Byte offset just past an autolink (`<https://…>`) or HTML tag (`<a …>`,
/// `</a>`) starting at `from`. Math such as `$a<b$` is not tag-shaped.
fn angle_span_end(line: &str, from: usize) -> Option<usize> {
    let after = &line[from + 1..];
    let rest = after.strip_prefix('/').unwrap_or(after);
    if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let name = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || "+.-".contains(c)))
        .unwrap_or(rest.len());
    if !rest[name..].starts_with([':', ' ', '\t', '/', '>']) {
        return None;
    }
    line[from..].find('>').map(|close| from + close + 1)
}

/// Byte offset just past a closing backtick run of exactly `len`, if any.
fn code_span_end(line: &str, from: usize, len: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if run == len {
                return Some(i + run);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

/// Byte offset of the next unescaped `$` on the line.
fn closing_dollar(line: &str, from: usize) -> Option<usize> {
    let mut i = from;
    while i < line.len() {
        let ch = line[i..].chars().next()?;
        match ch {
            '\\' => {
                i += 1;
                if i < line.len() {
                    i += line[i..].chars().next()?.len_utf8();
                }
            }
            '$' => return Some(i),
            _ => i += ch.len_utf8(),
        }
    }
    None
}

/// Rebuilds a `$...$` pair, trimming padding only when the content looks like
/// math. `$100 and then $` and `$ latex text here $` are prose, not formulas,
/// and so is `$100 > 50 $`: an amount right after the `$` reads as a price.
fn normalize_dollar_pair(body: &str) -> String {
    let trimmed = body.trim();
    let padded = body.chars().next().is_some_and(char::is_whitespace)
        || body.chars().last().is_some_and(char::is_whitespace);
    let prose_like = trimmed.chars().any(char::is_whitespace)
        && !trimmed.chars().any(|c| "\\{}^_=<>+-*/".contains(c));
    let price_like =
        body.starts_with(|c: char| c.is_ascii_digit()) && trimmed.chars().any(char::is_whitespace);
    if trimmed.is_empty() || !padded || prose_like || price_like {
        format!("${body}$")
    } else {
        format!("${trimmed}$")
    }
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
    fn loose_inline_math_delimiters_are_tolerated() {
        let o = with(|o| o.math = true);
        for md in [
            "$ L $",
            "$L $",
            "$ L$",
            "$ x^2 $",
            "$ a + b $",
            "$ \\frac{1}{2} $",
        ] {
            let html = render(md, &o).body;
            assert_eq!(html.matches("<math").count(), 1, "{md}: {html}");
            assert!(!html.contains('$'), "{md}: {html}");
        }
    }

    #[test]
    fn loose_math_keeps_prices_and_prose_literal() {
        let o = with(|o| o.math = true);
        let prose = render("價格 $100 元和 $200 元", &o).body;
        assert!(!prose.contains("<math"), "{prose}");
        assert!(prose.contains("$100 元和 $200 元"), "{prose}");

        let sentence = render("$ latex text here $", &o).body;
        assert!(!sentence.contains("<math"), "{sentence}");

        let mixed = render("$100 and then $x$", &o).body;
        assert_eq!(mixed.matches("<math").count(), 1, "{mixed}");
        assert!(mixed.contains("$100 and then"), "{mixed}");
    }

    #[test]
    fn closing_dollar_followed_by_a_digit_is_a_price() {
        let o = with(|o| o.math = true);
        for (md, kept) in [
            ("a $3.69$148 end", "$3.69$148"),
            ("a $3.69/1M）$148 end", "$3.69/1M）$148"),
            (
                "kimi $2.44 > glm $1.43 > x $1.42",
                "$2.44 &gt; glm $1.43 &gt; x $1.42",
            ),
        ] {
            let html = render(md, &o).body;
            assert!(!html.contains("<math"), "{md}: {html}");
            assert!(html.contains(kept), "{md}: {html}");
        }

        let bold = render("kimi-k3 **$3.6914/1M**（$148.99 / 40.4M tokens）", &o).body;
        assert!(!bold.contains("<math"), "{bold}");
        assert!(bold.contains("<strong>$3.6914/1M</strong>"), "{bold}");

        let math = render("$x$ 與 $y^2$ 2 個", &o).body;
        assert_eq!(math.matches("<math").count(), 2, "{math}");

        // URLs in autolinks and raw HTML are not prose: leave their `$` alone.
        let autolink = render("<https://x.com/a$1$2>", &o).body;
        assert!(
            autolink.contains("href=\"https://x.com/a$1$2\""),
            "{autolink}"
        );
        let tag = render("<a href=\"https://x.com/a$1$2\">t</a>", &o).body;
        assert!(tag.contains("href=\"https://x.com/a$1$2\""), "{tag}");
    }

    #[test]
    fn price_shaped_pairs_keep_their_padding() {
        let o = with(|o| o.math = true);
        let html = render("$100 > 50 $ end", &o).body;
        assert!(!html.contains("<math"), "{html}");
        assert!(html.contains("$100 &gt; 50 $ end"), "{html}");
    }

    #[test]
    fn loose_math_skips_code_escapes_and_display_math() {
        let o = with(|o| o.math = true);
        let html = render(
            "`$ L $` and ``$ x $``\n\n```\n$ L $\n```\n\n    $ L $\n\n\\$ L \\$\n\n$$\n x^2 \n$$\n",
            &o,
        )
        .body;
        assert!(html.contains("<code>$ L $</code>"), "{html}");
        assert!(html.contains("<code>$ x $</code>"), "{html}");
        assert!(html.contains("$ L $\n</code>"), "{html}");
        assert!(html.contains("display=\"block\""), "{html}");
        assert_eq!(html.matches("<math").count(), 1, "{html}");
    }

    #[test]
    fn adjacent_and_cjk_math_stay_intact() {
        let o = with(|o| o.math = true);
        let html = render("中文$L$中文 and $a$ and $b$", &o).body;
        assert_eq!(html.matches("<math").count(), 3, "{html}");
        assert!(html.contains("中文<math"), "{html}");
        assert!(html.contains("</math>中文"), "{html}");
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

    fn enhanced(md: &str) -> Rendered {
        render(
            md,
            &RenderOptions {
                enhance: true,
                ..Default::default()
            },
        )
    }

    #[test]
    fn enhance_is_off_by_default() {
        let html = body("- **A**：1\n- **B**：2\n\n| a |\n|-|\n| 1 |\n");
        assert!(!html.contains("class=\"kv\""), "{html}");
        assert!(!html.contains("table-wrap"), "{html}");
    }

    #[test]
    fn label_value_lists_become_property_grids() {
        let html = enhanced("- **背景**：說明\n- 頻道：TaG\n").body;
        assert!(html.contains("<ul class=\"kv\">"), "{html}");
        assert!(
            html.contains(
                "<div class=\"kv-key\"><strong>背景</strong></div><div class=\"kv-val\">說明</div>"
            ),
            "{html}"
        );
        assert!(
            html.contains("<div class=\"kv-key\">頻道</div><div class=\"kv-val\">TaG</div>"),
            "{html}"
        );
    }

    #[test]
    fn short_lists_become_cards_and_long_ones_stay() {
        let cards = enhanced("- one\n- two\n- three\n").body;
        assert!(cards.contains("<ul class=\"cards\">"), "{cards}");
        let steps = enhanced("1. one\n2. two\n3. three\n").body;
        assert!(steps.contains("<ol class=\"cards steps\">"), "{steps}");
        let long = "x".repeat(80);
        let plain = enhanced(&format!("- {long}\n- {long}\n- {long}\n")).body;
        assert!(plain.contains("<ul>"), "{plain}");
        let nested = enhanced("- a\n  - b\n- c\n- d\n").body;
        assert!(!nested.contains("cards"), "{nested}");
        let lead = "y".repeat(60);
        let findings = enhanced(&format!("1. **A** {lead}\n2. **B** {lead}\n3. {lead}\n")).body;
        assert!(findings.contains("<ol class=\"findings\">"), "{findings}");
        let labelled = enhanced(&format!(
            "1. **A**：{lead}\n2. **B**：{lead}\n3. **C**：{lead}\n"
        ))
        .body;
        assert!(labelled.contains("<ol class=\"findings\">"), "{labelled}");
        assert!(
            !labelled.contains("kv-key"),
            "numbered lists are never grids: {labelled}"
        );
    }

    #[test]
    fn tables_are_wrapped_and_tagged_by_shape() {
        let mut md = String::from("| # | Name | 查證 |\n|---|---|---|\n");
        for i in 1..=16 {
            md.push_str(&format!("| {i} | item {i} | ✅ ok |\n"));
        }
        let html = enhanced(&md).body;
        assert!(html.contains("<table>"), "table itself stays bare: {html}");
        let wrap = html.split("<div class=\"table-wrap ").nth(1).unwrap();
        let classes = &wrap[..wrap.find('"').unwrap()];
        for class in [
            "long",
            "sortable",
            "filterable",
            "resizable",
            "idx-first",
            "nowrap",
        ] {
            assert!(
                classes.split(' ').any(|c| c == class),
                "{class} missing in {classes}"
            );
        }
        assert!(
            html.contains("<span class=\"badge ok\">✅ ok</span>"),
            "{html}"
        );
        let r = enhanced("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(r.body.contains("table-wrap"), "{}", r.body);
        assert!(r.profile.interactive);
    }

    #[test]
    fn headings_get_cues_without_changing_ids_or_title() {
        let r = enhanced("# Notes — for the team\n\n## 0. 執行摘要\n\n### 2.1 背景\n");
        assert_eq!(r.title.as_deref(), Some("Notes — for the team"));
        let html = &r.body;
        assert!(
            html.contains("<span class=\"title-sub\">for the team</span>"),
            "{html}"
        );
        assert!(html.contains("class=\"icon-summary\""), "{html}");
        assert!(html.contains("id=\"0-執行摘要\""), "{html}");
        assert!(
            html.contains("<span class=\"h-num\">0</span>執行摘要"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"h-num\">2.1</span>背景"),
            "{html}"
        );
        assert!(
            html.contains("class=\"h-anchor\" href=\"#21-背景\""),
            "{html}"
        );
    }

    #[test]
    fn cjk_fullwidth_punctuation_emphasis_pairs() {
        // The closing `**` before a fullwidth colon (and the opening `**` after
        // a CJK character) are not flanking under plain CommonMark, so the whole
        // line shows its asterisks on GitHub. mhtml pairs them anyway.
        for (md, want) in [
            (
                "**審查執行：**2026-11-04，x",
                "<strong>審查執行：</strong>2026-11-04，x",
            ),
            ("**審查執行**：2026", "<strong>審查執行</strong>：2026"),
            ("**審查執行：** 2026", "<strong>審查執行：</strong> 2026"),
            ("這是**「引號」**文字", "這是<strong>「引號」</strong>文字"),
            // Two spots in one line must not cross-pair with each other.
            (
                "**審查：**x 這是**「引號」**文字",
                "<strong>審查：</strong>x 這是<strong>「引號」</strong>文字",
            ),
            (
                "**狀態：待修正**（2026-…）",
                "<strong>狀態：待修正</strong>（2026-…）",
            ),
        ] {
            let html = body(md);
            assert!(html.contains(want), "{md}: {html}");
            assert!(!html.contains("**"), "{md}: {html}");
        }
    }

    #[test]
    fn english_label_colon_emphasis_stays_literal() {
        // No CJK or fullwidth punctuation around the delimiters: CommonMark
        // behavior (literal asterisks) is kept.
        let html = body("**Note:**text");
        assert!(html.contains("**Note:**text"), "{html}");
        assert!(!html.contains("<strong>"), "{html}");
    }

    #[test]
    fn emphasis_fix_skips_code_spans_and_escapes() {
        let html = body("`**a：**b` 與 \\*x\\* 及 \\*\\*y");
        assert!(html.contains("<code>**a：**b</code>"), "{html}");
        assert!(!html.contains("<em>"), "{html}");
        assert!(!html.contains("<strong>"), "{html}");
        assert!(html.contains("*x*"), "{html}");
        assert!(html.contains("**y"), "{html}");
    }

    #[test]
    fn emphasis_fix_works_in_tables_and_headings_with_stable_ids() {
        let html = body("## **標題：**副\n\n| 欄 | 值 |\n|---|---|\n| **狀態：**x | y |\n");
        // The id is the GitHub slug of the heading text either way.
        assert!(
            html.contains("<h2 id=\"標題副\"><strong>標題：</strong>副</h2>"),
            "{html}"
        );
        assert!(html.contains("<strong>狀態：</strong>x"), "{html}");
    }

    #[test]
    fn fix_comments_never_reach_the_output() {
        let md = "**審查：**x 這是**「引號」**文字\n\n原始 <!----> 註解也移除\n";
        for unsafe_html in [false, true] {
            let html = render(
                md,
                &RenderOptions {
                    unsafe_html,
                    ..Default::default()
                },
            )
            .body;
            assert!(!html.contains("<!---->"), "unsafe={unsafe_html}: {html}");
            assert!(html.contains("<strong>審查：</strong>x"), "{html}");
        }
    }

    #[test]
    fn cjk_label_fix_adds_exactly_the_expected_strong_tags() {
        // Four `**label:**value` spots plus two plain bolds: exactly four new
        // <strong> elements, nothing else changes.
        let md = "\
# 標題

**正常一** 與 **正常二** 保持原樣。

**審查執行：**2026-11-04

這是**「引號」**文字

| 欄 | 值 |
|---|---|
| **狀態：**待確認 | x |

**注意：**完畢
";
        let html = body(md);
        assert_eq!(html.matches("<strong>").count(), 6, "{html}");
        for want in [
            "<strong>正常一</strong>",
            "<strong>正常二</strong>",
            "<strong>審查執行：</strong>2026-11-04",
            "這是<strong>「引號」</strong>文字",
            "<strong>狀態：</strong>待確認",
            "<strong>注意：</strong>完畢",
        ] {
            assert!(html.contains(want), "missing {want}: {html}");
        }
    }

    #[test]
    fn unpaired_emphasis_is_diagnosed_with_line_numbers() {
        let md = "\
# 狀態報告

以下是本週的重點整理。
**審查執行：**2026-11-04，全數通過
這是**「引號」**文字，行內還有 `**code：**x` 程式碼。

- **正常**：項目
- 星號可以跳脫 \\* 與 a * b * c 數學寫法

| 欄位 | 值 |
| --- | --- |
| **狀態：**待確認 | ok |
| plain | ok |

## 章節

一般段落。

> 引用沒有問題。

**注意：**表格已更新

結尾。
";
        let r = render(md, &RenderOptions::default());
        // Bold on screen or not (GitHub shows these markers literally), every
        // unpaired emphasis attempt is reported once with its line.
        let lines: Vec<usize> = r.warnings.iter().map(|w| w.line).collect();
        assert_eq!(lines, vec![4, 5, 12, 21], "{:?}", r.warnings);
        assert!(
            r.warnings.iter().all(|w| w.marker == "**"),
            "{:?}",
            r.warnings
        );
    }

    #[test]
    fn clean_documents_have_no_emphasis_warnings() {
        for md in [
            "**正常** 與 *斜體*",
            "**審查執行：** 2026",
            "`**a：**b`",
            "\\* 星 號 \\*",
            "a * b * c",
            "2 ** 3 = 6",
            "```\n**code：**x\n```\n",
        ] {
            let r = render(md, &RenderOptions::default());
            assert!(r.warnings.is_empty(), "{md}: {:?}", r.warnings);
        }
    }

    #[test]
    fn document_profile_drives_layout() {
        let mut md = String::from("# Guide\n\n");
        for i in 1..=6 {
            md.push_str(&format!(
                "## Part {i}\n\ntext\n\n### Detail {i}\n\nmore\n\n"
            ));
        }
        let r = enhanced(&md);
        assert!(
            r.profile.classes.contains(&"doc-reference"),
            "{:?}",
            r.profile
        );
        let outline = r.outline.expect("outline for a long document");
        assert!(outline.contains("class=\"toc toc-side\""), "{outline}");
        assert!(outline.contains("href=\"#detail-6\""), "{outline}");

        let prose: String = (0..10)
            .map(|i| format!("第 {i} 段，大家好，這是逐字稿。\n\n"))
            .collect();
        let r = enhanced(&format!("———— 會議開始 13:00 ————\n\n{prose}"));
        assert!(r.profile.classes.contains(&"doc-prose"), "{:?}", r.profile);
        assert!(r.profile.classes.contains(&"is-cjk"));
        assert!(r.outline.is_none() && !r.has_h1);
        assert!(
            r.body
                .contains("<p class=\"divider-label\"><span>會議開始 13:00</span></p>"),
            "{}",
            r.body
        );
    }
}
