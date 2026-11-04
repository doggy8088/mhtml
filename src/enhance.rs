//! Adaptive presentation: reads the structure of the document and tags it with
//! classes the built-in "enhance" stylesheet and script understand.
//!
//! Everything here only adds classes and wrapper elements; the text, heading ids
//! and document order stay exactly as written. The pass runs on the event
//! stream after heading ids are assigned, so slugs, the page title and the TOC
//! never see the decorations. Classes survive sanitizing; anything that needs an
//! attribute the sanitizer drops (`role`, `tabindex`, `data-*`) is added by the
//! page script instead.

use std::collections::HashMap;

use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

use crate::page::escape;

/// What the document looks like as a whole.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// Classes for `<main>`, e.g. `doc-reference is-cjk`.
    pub classes: Vec<&'static str>,
    /// Long, multi-section document: worth a sticky outline on wide screens.
    pub outline: bool,
    /// At least one table, list or code block that the page script enhances.
    pub interactive: bool,
}

#[derive(Debug, Default)]
struct Stats {
    h2: usize,
    h3: usize,
    paragraphs: usize,
    tables: usize,
    wide_tables: bool,
    code_blocks: usize,
    text_chars: usize,
    cjk_chars: usize,
    latin_chars: usize,
}

/// Runs every pass and describes the document.
pub fn enhance<'a>(events: Vec<Event<'a>>) -> (Vec<Event<'a>>, Profile) {
    let mut stats = Stats::default();
    count(&events, &mut stats);

    let cjk = is_cjk_text(stats.cjk_chars, stats.latin_chars);
    let events = decorate_headings(events);
    let events = classify_lists(events);
    let events = wrap_tables(events, &mut stats);
    let events = mark_dividers(events);
    let events = add_reading_time(events, &stats, cjk);

    let headings = stats.h2 + stats.h3;
    let mut classes = Vec::new();
    if headings == 0 && stats.tables == 0 && stats.paragraphs >= 8 {
        classes.push("doc-prose");
    } else if headings >= 10 || stats.tables >= 6 {
        classes.push("doc-reference");
    }
    if stats.wide_tables {
        classes.push("doc-wide");
    }
    if stats.text_chars >= 6000 {
        classes.push("doc-long");
    }
    if cjk {
        classes.push("is-cjk");
    }
    let profile = Profile {
        classes,
        outline: stats.h2 >= 3 && headings >= 8,
        interactive: stats.tables > 0 || stats.code_blocks > 0,
    };
    (events, profile)
}

fn count(events: &[Event<'_>], stats: &mut Stats) {
    for event in events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => match level {
                HeadingLevel::H2 => stats.h2 += 1,
                HeadingLevel::H3 => stats.h3 += 1,
                _ => {}
            },
            Event::Start(Tag::Paragraph) => stats.paragraphs += 1,
            Event::Start(Tag::CodeBlock(_)) => stats.code_blocks += 1,
            Event::Html(html) if html.starts_with("<pre") => stats.code_blocks += 1,
            Event::Text(t) | Event::Code(t) => {
                for c in t.chars() {
                    stats.text_chars += 1;
                    if is_cjk_char(c) {
                        stats.cjk_chars += 1;
                    } else if c.is_ascii_alphabetic() {
                        stats.latin_chars += 1;
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_cjk_char(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30ff      // kana
        | 0x3400..=0x4dbf    // CJK extension A
        | 0x4e00..=0x9fff    // CJK unified ideographs
        | 0xac00..=0xd7af    // hangul
        | 0xf900..=0xfaff)
}

/// Technical CJK notes carry a lot of Latin letters (product names, code,
/// URLs), so one CJK character is weighed like about eight letters.
fn is_cjk_text(cjk: usize, latin: usize) -> bool {
    cjk >= 20 && cjk * 8 >= latin
}

/// Best-effort `<html lang>` for CJK documents, so the browser picks the right
/// glyph variants (Han unification) and line-breaking rules. `None` means "no
/// strong signal", and the caller keeps its default.
pub fn detect_lang(source: &str) -> Option<&'static str> {
    const TRADITIONAL: &str =
        "這們來個說為時會與國學對開發經動現進體點實問題關係應該種讓頁檔轉們標準議錄檢驗證書廠";
    const SIMPLIFIED: &str =
        "这们来个说为时会与国学对开发经动现进体点实问题关系应该种让页档转们标准议录检验证书厂";
    let (mut han, mut kana, mut hangul, mut latin, mut trad, mut simp) = (0, 0, 0, 0, 0, 0);
    for c in source.chars() {
        match c as u32 {
            0x3040..=0x30ff => kana += 1,
            0xac00..=0xd7af => hangul += 1,
            0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff => {
                han += 1;
                if TRADITIONAL.contains(c) {
                    trad += 1;
                } else if SIMPLIFIED.contains(c) {
                    simp += 1;
                }
            }
            _ if c.is_ascii_alphabetic() => latin += 1,
            _ => {}
        }
    }
    if !is_cjk_text(han + kana + hangul, latin) {
        return None;
    }
    if hangul > han + kana {
        Some("ko")
    } else if kana * 20 > han {
        Some("ja")
    } else if trad > simp {
        Some("zh-Hant")
    } else if simp > trad {
        Some("zh-Hans")
    } else {
        None
    }
}

// --- headings -------------------------------------------------------------

/// Section icons, matched in order against the H2 text (lower-cased). They are
/// drawn by CSS (`h2.icon-*::before`), so the heading text is untouched.
const ICONS: &[(&str, &[&str])] = &[
    (
        "summary",
        &[
            "摘要", "結論", "重點", "總結", "summary", "overview", "tl;dr", "結語",
        ],
    ),
    (
        "info",
        &["文件資訊", "文件用途", "關於", "about", "metadata"],
    ),
    (
        "todo",
        &[
            "待查",
            "待辦",
            "待確認",
            "缺口",
            "下一步",
            "todo",
            "next step",
            "action",
            "known gap",
            "清單",
        ],
    ),
    (
        "risk",
        &[
            "風險", "注意", "限制", "陷阱", "risk", "warning", "caveat", "pitfall",
        ],
    ),
    (
        "tip",
        &[
            "建議",
            "技巧",
            "最佳實務",
            "tip",
            "best practice",
            "recommend",
        ],
    ),
    (
        "perf",
        &[
            "效能",
            "調校",
            "速度",
            "瓶頸",
            "performance",
            "tuning",
            "latency",
        ],
    ),
    (
        "trouble",
        &[
            "疑難",
            "排錯",
            "排解",
            "錯誤",
            "診斷",
            "troubleshoot",
            "debug",
            "faq",
            "問題",
        ],
    ),
    (
        "schedule",
        &[
            "時程", "排程", "週", "路線", "計畫", "規劃", "schedule", "timeline", "roadmap", "plan",
        ],
    ),
    ("video", &["影片", "video", "youtube", "頻道"]),
    (
        "people",
        &["角色", "權限", "帳號", "組織", "role", "permission", "team"],
    ),
    ("security", &["資安", "安全", "security", "授權", "license"]),
    (
        "deploy",
        &[
            "上線", "部署", "安裝", "環境", "deploy", "install", "setup", "rollback",
        ],
    ),
    ("test", &["測試", "驗收", "驗證", "test", "qa"]),
    ("tool", &["工具", "外掛", "tool", "plugin", "utility"]),
    (
        "arch",
        &[
            "架構",
            "流程",
            "設計",
            "整合",
            "architecture",
            "flow",
            "design",
            "integration",
        ],
    ),
    (
        "code",
        &[
            "api", "程式", "開發", "built-in", "trigger", "sdk", "code", "語法",
        ],
    ),
    (
        "history",
        &[
            "歷史",
            "版本",
            "生命週期",
            "支援",
            "history",
            "version",
            "lifecycle",
        ],
    ),
    (
        "goal",
        &[
            "背景",
            "目標",
            "範圍",
            "需求",
            "background",
            "goal",
            "scope",
            "requirement",
        ],
    ),
    (
        "refs",
        &[
            "參考",
            "來源",
            "資源",
            "文件",
            "手冊",
            "書",
            "學習",
            "閱讀順序",
            "reference",
            "resource",
            "source",
            "docs",
            "learning",
        ],
    ),
    ("appendix", &["附錄", "附註", "附件", "appendix", "note"]),
];

/// Adds the subtitle split to the first H1, number badges to H2/H3, an icon
/// class to H2 and a permalink to H2/H3.
fn decorate_headings(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out = Vec::with_capacity(events.len() + 16);
    let mut seen_h1 = false;
    let mut iter = events.into_iter().peekable();

    while let Some(event) = iter.next() {
        let Event::Start(Tag::Heading {
            level,
            id,
            mut classes,
            attrs,
        }) = event
        else {
            out.push(event);
            continue;
        };

        let mut inner = Vec::new();
        for e in iter.by_ref() {
            if matches!(e, Event::End(TagEnd::Heading(_))) {
                break;
            }
            inner.push(e);
        }
        let text: String = inner
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) | Event::Code(t) => Some(t.as_ref()),
                _ => None,
            })
            .collect();

        match level {
            HeadingLevel::H1 if !seen_h1 => {
                seen_h1 = true;
                inner = split_subtitle(inner);
            }
            HeadingLevel::H2 | HeadingLevel::H3 => {
                if let Some(Event::Text(first)) = inner.first()
                    && let Some((badge, rest)) = number_badge(first)
                {
                    let rest = rest.to_string();
                    inner[0] = Event::Text(CowStr::from(rest));
                    inner.insert(
                        0,
                        Event::Html(CowStr::from(format!(
                            "<span class=\"h-num\">{}</span>",
                            escape(&badge)
                        ))),
                    );
                }
                if level == HeadingLevel::H2
                    && !starts_with_emoji(&text)
                    && let Some(icon) = icon_for(&text)
                {
                    classes.push(CowStr::from(format!("icon-{icon}")));
                }
                if let Some(id) = &id {
                    inner.push(Event::Html(CowStr::from(format!(
                        "<a class=\"h-anchor\" href=\"#{}\" title=\"Permalink\"></a>",
                        escape(id)
                    ))));
                }
            }
            _ => {}
        }

        out.push(Event::Start(Tag::Heading {
            level,
            id,
            classes,
            attrs,
        }));
        out.extend(inner);
        out.push(Event::End(TagEnd::Heading(level)));
    }
    out
}

/// `Title — subtitle` becomes a title with a smaller subtitle line.
fn split_subtitle(inner: Vec<Event<'_>>) -> Vec<Event<'_>> {
    const SEPARATORS: &[&str] = &[" — ", " ── ", "——", " – ", " | "];
    let Some((at, sep_pos, sep)) = inner.iter().enumerate().find_map(|(i, e)| match e {
        Event::Text(t) => SEPARATORS
            .iter()
            .filter_map(|s| t.find(s).map(|p| (i, p, *s)))
            .min_by_key(|&(_, p, _)| p),
        _ => None,
    }) else {
        return inner;
    };
    let Event::Text(t) = &inner[at] else {
        return inner;
    };
    let (head, tail) = (
        t[..sep_pos].trim_end().to_string(),
        t[sep_pos + sep.len()..].trim_start().to_string(),
    );
    let has_more = inner.len() > at + 1;
    if head.is_empty() && at == 0 || tail.is_empty() && !has_more {
        return inner;
    }

    let mut out = Vec::with_capacity(inner.len() + 3);
    let mut rest = inner.into_iter();
    out.extend(rest.by_ref().take(at));
    rest.next(); // the split text event
    out.push(Event::Text(CowStr::from(head)));
    out.push(Event::Html(CowStr::from(
        "<span class=\"title-sep\"> — </span><span class=\"title-sub\">",
    )));
    if !tail.is_empty() {
        out.push(Event::Text(CowStr::from(tail)));
    }
    out.extend(rest);
    out.push(Event::Html(CowStr::from("</span>")));
    out
}

/// `0. 執行摘要`, `2.1 背景`, `P1. Oracle ...` → (`0`, `執行摘要`), ...
/// A bare `10 tips` is left alone: it needs a trailing `.`/`、` or an inner dot.
fn number_badge(text: &str) -> Option<(String, &str)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 2 && bytes[i].is_ascii_uppercase() {
        i += 1;
    }
    let digits_start = i;
    let mut inner_dot = false;
    loop {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            inner_dot = true;
            i += 1;
            continue;
        }
        break;
    }
    if i - digits_start > 4 {
        return None; // a year or a part number, not a section number
    }
    let badge = text[..i].to_string();
    let rest = &text[i..];
    let rest = if let Some(r) = rest
        .strip_prefix('.')
        .or_else(|| rest.strip_prefix('、'))
        .or_else(|| rest.strip_prefix('．'))
    {
        r
    } else if inner_dot {
        rest
    } else {
        return None;
    };
    let trimmed = rest.trim_start();
    // After a plain `.` a space is required, so `1.5x faster` stays as written.
    if trimmed.is_empty() || (trimmed.len() == rest.len() && !rest.starts_with(is_cjk_char)) {
        return None;
    }
    Some((badge, trimmed))
}

fn starts_with_emoji(text: &str) -> bool {
    text.trim_start().chars().next().is_some_and(
        |c| matches!(c as u32, 0x2190..=0x2bff | 0x1f000..=0x1faff | 0x3297 | 0x3299 | 0x203b),
    )
}

fn icon_for(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    ICONS
        .iter()
        .find(|(_, words)| words.iter().any(|w| lower.contains(w)))
        .map(|(name, _)| *name)
}

// --- lists ----------------------------------------------------------------

#[derive(Debug, Default)]
struct Item {
    start: usize,
    end: usize,
    chars: usize,
    nested_list: bool,
    block: bool,
    task: bool,
    loose: bool,
    starts_strong: bool,
    key: Option<KeySplit>,
}

/// Where an item's leading label ends.
#[derive(Debug, Clone, Copy)]
enum KeySplit {
    /// `**Label**：value` — the strong element ends at this index; when the next
    /// text starts with a colon, that colon is dropped.
    Strong { strong_end: usize },
    /// `Label：value` in the first text event, split at this byte offset (the
    /// offset of the colon).
    Text { at: usize, colon_len: usize },
}

/// Replacement events per input index (the index's own event included).
type Plan<'a> = HashMap<usize, Vec<Event<'a>>>;

fn classify_lists(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut plan: Plan = HashMap::new();
    let mut list_depth = 0usize;
    for i in 0..events.len() {
        match &events[i] {
            Event::Start(Tag::List(first)) => {
                if let Some(kind) = classify_list(&events, i, list_depth, &mut plan) {
                    let open = match first {
                        None => format!("<ul class=\"{kind}\">\n"),
                        Some(_) => format!("<ol class=\"{kind}\">\n"),
                    };
                    plan.insert(i, vec![Event::Html(CowStr::from(open))]);
                }
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            _ => {}
        }
    }
    apply(events, plan)
}

fn classify_list<'a>(
    events: &[Event<'a>],
    start: usize,
    list_depth: usize,
    plan: &mut Plan<'a>,
) -> Option<&'static str> {
    let ordered_from = match &events[start] {
        Event::Start(Tag::List(first)) => *first,
        _ => return None,
    };
    let items = list_items(events, start);
    let n = items.len();
    if n < 2 {
        return None;
    }

    // `Label：value` on every item: a property grid.
    // Numbered lists keep their numbers, so only bullets become a grid.
    if ordered_from.is_none() && items.iter().all(|it| it.key.is_some() && !it.loose) {
        for it in &items {
            plan_key_value(events, it, plan);
        }
        return Some("kv");
    }

    let flat = items
        .iter()
        .all(|it| !it.nested_list && !it.block && !it.task);
    let numbered_from_one = ordered_from.is_none_or(|n| n == 1);

    // Numbered findings: long points, most of them opening with a bold lead.
    let leads = items.iter().filter(|it| it.starts_strong).count();
    if ordered_from.is_some()
        && list_depth == 0
        && numbered_from_one
        && n >= 3
        && leads * 2 >= n
        && items.iter().all(|it| !it.block)
        && items.iter().map(|it| it.chars).sum::<usize>() >= 40 * n
    {
        return Some("findings");
    }

    // A few short, flat items read better as cards.
    let total: usize = items.iter().map(|it| it.chars).sum();
    if list_depth == 0
        && flat
        && (3..=8).contains(&n)
        && items.iter().all(|it| it.chars <= 60)
        && total <= 320
    {
        return Some(match ordered_from {
            Some(_) if numbered_from_one => "cards steps",
            Some(_) => return None,
            None => "cards",
        });
    }
    None
}

/// The direct items of the list that starts at `start`.
fn list_items(events: &[Event<'_>], start: usize) -> Vec<Item> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut current: Option<Item> = None;
    let mut first_inline_seen = false;
    let mut strong_depth = 0usize;
    let mut i = start;
    while i < events.len() {
        let event = &events[i];
        match event {
            Event::Start(Tag::List(_)) => {
                depth += 1;
                if depth > 1
                    && let Some(it) = current.as_mut()
                {
                    it.nested_list = true;
                }
            }
            Event::End(TagEnd::List(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Event::Start(Tag::Item) if depth == 1 => {
                current = Some(Item {
                    start: i,
                    ..Default::default()
                });
                first_inline_seen = false;
            }
            Event::End(TagEnd::Item) if depth == 1 => {
                if let Some(mut it) = current.take() {
                    it.end = i;
                    items.push(it);
                }
            }
            _ if depth == 1 => {
                if let Some(it) = current.as_mut() {
                    inspect_item_event(events, i, it, &mut first_inline_seen, &mut strong_depth);
                }
            }
            _ => {
                if let Some(it) = current.as_mut()
                    && let Event::Text(t) | Event::Code(t) = event
                {
                    it.chars += t.chars().count();
                }
            }
        }
        i += 1;
    }
    items
}

fn inspect_item_event(
    events: &[Event<'_>],
    i: usize,
    it: &mut Item,
    first_inline_seen: &mut bool,
    strong_depth: &mut usize,
) {
    let event = &events[i];
    match event {
        Event::Start(Tag::Paragraph) if i == it.start + 1 => it.loose = true,
        Event::Start(Tag::CodeBlock(_) | Tag::Table(_) | Tag::BlockQuote(_)) => it.block = true,
        Event::Html(h) if h.contains("<pre") || h.contains("<div") || h.contains("<table") => {
            it.block = true
        }
        Event::TaskListMarker(_) => it.task = true,
        _ => {}
    }
    if let Event::Text(t) | Event::Code(t) = event {
        it.chars += t.chars().count();
    }

    if *first_inline_seen {
        if *strong_depth > 0 {
            match event {
                Event::Start(Tag::Strong) => *strong_depth += 1,
                Event::End(TagEnd::Strong) => {
                    *strong_depth -= 1;
                    if *strong_depth == 0 {
                        it.key = strong_key(events, i);
                    }
                }
                _ => {}
            }
        }
        return;
    }
    match event {
        Event::Start(Tag::Paragraph) => {}
        Event::Start(Tag::Strong) => {
            *first_inline_seen = true;
            it.starts_strong = true;
            *strong_depth = 1;
        }
        Event::Text(t) => {
            *first_inline_seen = true;
            it.key = text_key(t);
        }
        _ => *first_inline_seen = true,
    }
}

/// `**Label**` followed by `：…` (or a label that already ends in a colon).
fn strong_key(events: &[Event<'_>], strong_end: usize) -> Option<KeySplit> {
    let label: String = events[..strong_end]
        .iter()
        .rev()
        .take_while(|e| !matches!(e, Event::Start(Tag::Strong)))
        .filter_map(|e| match e {
            Event::Text(t) | Event::Code(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect();
    let label_len = label.chars().count();
    if label_len == 0 || label_len > 24 {
        return None;
    }
    let colon_inside = label.trim_end().ends_with(['：', ':']);
    let colon_after =
        matches!(events.get(strong_end + 1), Some(Event::Text(t)) if t.starts_with(['：', ':']));
    (colon_inside || colon_after).then_some(KeySplit::Strong { strong_end })
}

/// `頻道：Learn with TaG` — a short label before a full-width (or `: `) colon.
fn text_key(text: &str) -> Option<KeySplit> {
    let (at, colon_len) = match (text.find('：'), text.find(": ")) {
        (Some(a), Some(b)) => {
            if a < b {
                (a, '：'.len_utf8())
            } else {
                (b, 1)
            }
        }
        (Some(a), None) => (a, '：'.len_utf8()),
        (None, Some(b)) => (b, 1),
        (None, None) => return None,
    };
    let label = &text[..at];
    let chars = label.chars().count();
    let ok = (1..=12).contains(&chars)
        && !label.contains(['。', '，', ',', '(', '（', '「', '`'])
        && label.split_whitespace().count() <= 3;
    ok.then_some(KeySplit::Text { at, colon_len })
}

fn plan_key_value<'a>(events: &[Event<'a>], it: &Item, plan: &mut Plan<'a>) {
    let open_key = || Event::Html(CowStr::from("<div class=\"kv-key\">"));
    let open_val = || Event::Html(CowStr::from("</div><div class=\"kv-val\">"));
    match it.key {
        Some(KeySplit::Strong { strong_end }) => {
            plan.insert(it.start, vec![events[it.start].clone(), open_key()]);
            plan.insert(strong_end, vec![events[strong_end].clone(), open_val()]);
            if let Some(Event::Text(t)) = events.get(strong_end + 1)
                && let Some(rest) = t.strip_prefix(['：', ':'])
            {
                let rest = rest.trim_start().to_string();
                let mut replacement = vec![];
                if !rest.is_empty() {
                    replacement.push(Event::Text(CowStr::from(rest)));
                }
                plan.insert(strong_end + 1, replacement);
            }
        }
        Some(KeySplit::Text { at, colon_len }) => {
            let Some(Event::Text(t)) = events.get(it.start + 1) else {
                return;
            };
            let label = t[..at].trim().to_string();
            let rest = t[at + colon_len..].trim_start().to_string();
            let mut replacement = vec![Event::Html(CowStr::from(format!(
                "<div class=\"kv-key\">{}</div><div class=\"kv-val\">",
                escape(&label)
            )))];
            if !rest.is_empty() {
                replacement.push(Event::Text(CowStr::from(rest)));
            }
            plan.insert(it.start + 1, replacement);
        }
        None => return,
    }
    plan.insert(
        it.end,
        vec![Event::Html(CowStr::from("</div>")), events[it.end].clone()],
    );
}

fn apply<'a>(events: Vec<Event<'a>>, mut plan: Plan<'a>) -> Vec<Event<'a>> {
    if plan.is_empty() {
        return events;
    }
    let mut out = Vec::with_capacity(events.len() + plan.len() * 2);
    for (i, event) in events.into_iter().enumerate() {
        match plan.remove(&i) {
            Some(replacement) => out.extend(replacement),
            None => out.push(event),
        }
    }
    out
}

// --- tables ---------------------------------------------------------------

/// Status markers used by research notes (verified / partial / unverified /
/// third party) and their badge classes.
const STATUS: &[(&str, &str)] = &[
    ("✅", "ok"),
    ("✔", "ok"),
    ("⚠", "warn"),
    ("❌", "bad"),
    ("✖", "bad"),
    ("🔴", "bad"),
    ("※", "info"),
];

fn wrap_tables<'a>(events: Vec<Event<'a>>, stats: &mut Stats) -> Vec<Event<'a>> {
    let mut plan: Plan = HashMap::new();
    let mut i = 0;
    while i < events.len() {
        if let Event::Start(Tag::Table(aligns)) = &events[i] {
            let cols = aligns.len();
            let end = (i..events.len())
                .find(|&j| matches!(events[j], Event::End(TagEnd::Table)))
                .unwrap_or(events.len() - 1);
            let classes = table_classes(&events[i..=end], cols, i, &mut plan);
            stats.tables += 1;
            if classes.contains(&"wide") {
                stats.wide_tables = true;
            }
            plan.insert(
                i,
                vec![
                    Event::Html(CowStr::from(format!(
                        "<div class=\"table-wrap {}\">\n",
                        classes.join(" ")
                    ))),
                    events[i].clone(),
                ],
            );
            plan.insert(
                end,
                vec![events[end].clone(), Event::Html(CowStr::from("</div>\n"))],
            );
            i = end;
        }
        i += 1;
    }
    apply(events, plan)
}

fn table_classes<'a>(
    table: &[Event<'a>],
    cols: usize,
    offset: usize,
    plan: &mut Plan<'a>,
) -> Vec<&'static str> {
    let mut headers: Vec<String> = Vec::new();
    let mut rows = 0usize;
    let mut cell_lens: Vec<usize> = Vec::new();
    let mut first_col_lens: Vec<usize> = Vec::new();
    let mut first_col_numeric = true;
    let mut in_head = false;
    let mut cell: Option<(usize, String, bool)> = None; // (start index, text, simple)
    let mut col = 0usize;

    for (k, event) in table.iter().enumerate() {
        match event {
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => in_head = false,
            Event::Start(Tag::TableRow) => {
                rows += 1;
                col = 0;
            }
            Event::Start(Tag::TableCell) => cell = Some((k, String::new(), true)),
            Event::End(TagEnd::TableCell) => {
                if let Some((start, text, simple)) = cell.take() {
                    let text = text.trim().to_string();
                    let len = text.chars().count();
                    if in_head {
                        headers.push(text);
                    } else {
                        cell_lens.push(len);
                        if col == 0 {
                            first_col_lens.push(len);
                            first_col_numeric &= !text.is_empty()
                                && len <= 4
                                && text.chars().all(|c| c.is_ascii_digit());
                        }
                        if let Some(kind) = status_kind(&text)
                            && simple
                            && len <= 28
                        {
                            plan.insert(
                                offset + start,
                                vec![
                                    table[start].clone(),
                                    Event::Html(CowStr::from(format!(
                                        "<span class=\"badge {kind}\">"
                                    ))),
                                ],
                            );
                            plan.insert(
                                offset + k,
                                vec![Event::Html(CowStr::from("</span>")), event.clone()],
                            );
                        }
                    }
                    col += 1;
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, text, _)) = cell.as_mut() {
                    text.push_str(t);
                }
            }
            Event::Html(_) | Event::InlineHtml(_) | Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, simple)) = cell.as_mut() {
                    *simple = false;
                }
            }
            _ => {}
        }
    }

    let avg = |lens: &[usize]| {
        if lens.is_empty() {
            0
        } else {
            lens.iter().sum::<usize>() / lens.len()
        }
    };
    let avg_cell = avg(&cell_lens);
    let first_avg = avg(&first_col_lens);
    let rest: Vec<usize> = cell_lens
        .iter()
        .enumerate()
        .filter(|(n, _)| cols > 0 && n % cols != 0)
        .map(|(_, &l)| l)
        .collect();
    let rest_avg = avg(&rest);

    let mut classes = Vec::new();
    if cols >= 6 || (cols >= 4 && avg_cell >= 40) {
        classes.push("wide");
    }
    if rows >= 15 {
        classes.push("long");
    }
    // 2 列以上的資料列即可排序（原本需 5 列，小表格看不到排序控制項）
    if rows >= 2 {
        classes.push("sortable");
    }
    if rows >= 12 {
        classes.push("filterable");
    }
    if cols >= 3 {
        classes.push("resizable");
    }
    let max_cell = cell_lens.iter().copied().max().unwrap_or(0);
    if avg_cell <= 14 && max_cell <= 48 {
        classes.push("compact");
    } else if avg_cell >= 40 {
        classes.push("prose");
    }
    // Short cells read best on one line (the wrapper scrolls if needed).
    let nowrap = max_cell <= 24;
    if nowrap {
        classes.push("nowrap");
    }
    if cols <= 4 && avg_cell >= 16 && !nowrap {
        classes.push("stackable");
    }
    if (2..=3).contains(&cols) && first_avg <= 16 && rest_avg >= 2 * first_avg.max(8) {
        classes.push("key-first");
    }
    let index_header = headers.first().is_some_and(|h| {
        matches!(
            h.as_str(),
            "#" | "No." | "No" | "序" | "序號" | "順序" | "項次" | "編號"
        )
    });
    if index_header || (rows >= 3 && first_col_numeric) {
        classes.push("idx-first");
    }
    classes
}

fn status_kind(text: &str) -> Option<&'static str> {
    STATUS
        .iter()
        .find(|(mark, _)| text.starts_with(mark))
        .map(|(_, kind)| *kind)
}

// --- paragraphs -----------------------------------------------------------

/// `———— 會議開始 13:00 ————` style marker paragraphs become a centered label.
fn mark_dividers(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut plan: Plan = HashMap::new();
    for i in 0..events.len().saturating_sub(2) {
        let (Event::Start(Tag::Paragraph), Event::Text(t), Event::End(TagEnd::Paragraph)) =
            (&events[i], &events[i + 1], &events[i + 2])
        else {
            continue;
        };
        let rule = |c: char| matches!(c, '—' | '─' | '-' | '=' | '━' | '–');
        let label = t.trim().trim_matches(rule).trim();
        let marked = t.trim().starts_with(rule)
            && t.trim().ends_with(rule)
            && t.trim().chars().take_while(|&c| rule(c)).count() >= 2;
        if !marked || label.is_empty() || label.chars().count() > 60 {
            continue;
        }
        plan.insert(
            i,
            vec![Event::Html(CowStr::from(format!(
                "<p class=\"divider-label\"><span>{}</span></p>\n",
                escape(label)
            )))],
        );
        plan.insert(i + 1, vec![]);
        plan.insert(i + 2, vec![]);
    }
    apply(events, plan)
}

/// A short "≈ N min read" line right after the page title of longer documents.
fn add_reading_time<'a>(events: Vec<Event<'a>>, stats: &Stats, cjk: bool) -> Vec<Event<'a>> {
    if stats.text_chars < 1500 {
        return events;
    }
    let latin_words = stats.latin_chars / 5;
    let minutes = (stats.cjk_chars / 400 + latin_words / 220).max(1);
    let line = if cjk {
        format!(
            "<p class=\"doc-meta\"><span>約 {minutes} 分鐘閱讀</span><span>{} 字</span></p>\n",
            group_thousands(stats.cjk_chars + latin_words)
        )
    } else {
        format!(
            "<p class=\"doc-meta\"><span>{minutes} min read</span><span>{} words</span></p>\n",
            group_thousands(stats.latin_chars / 5 + stats.cjk_chars)
        )
    };
    let mut out = events;
    let at = out
        .iter()
        .position(|e| matches!(e, Event::End(TagEnd::Heading(HeadingLevel::H1))));
    // Only when the document opens with its title.
    if let Some(at) = at
        && matches!(
            out.first(),
            Some(Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }))
        )
    {
        out.insert(at + 1, Event::Html(CowStr::from(line)));
    }
    out
}

fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_badges_need_a_section_number() {
        assert_eq!(number_badge("0. 執行摘要"), Some(("0".into(), "執行摘要")));
        assert_eq!(number_badge("2.1 背景"), Some(("2.1".into(), "背景")));
        assert_eq!(number_badge("P1. Oracle"), Some(("P1".into(), "Oracle")));
        assert_eq!(number_badge("3、範圍"), Some(("3".into(), "範圍")));
        assert_eq!(number_badge("10 tips"), None);
        assert_eq!(number_badge("1.5x faster"), None);
        assert_eq!(number_badge("2026-11-04 notes"), None);
        assert_eq!(number_badge("Intro"), None);
        assert_eq!(number_badge("1."), None);
    }

    #[test]
    fn text_labels_are_short_and_clean() {
        assert!(text_key("頻道：Learn with TaG").is_some());
        assert!(text_key("Channel: TaG").is_some());
        assert!(text_key("這是一段很長的敘述，而不是標籤：所以不算").is_none());
        assert!(text_key("no colon here").is_none());
    }

    #[test]
    fn languages_are_detected_from_script() {
        assert_eq!(
            detect_lang("這是繁體中文的文件，說明開發流程與專案背景，讓工程師了解需求。"),
            Some("zh-Hant")
        );
        assert_eq!(
            detect_lang("这是简体中文的文件，说明开发流程与项目背景，让工程师了解需求。"),
            Some("zh-Hans")
        );
        assert_eq!(
            detect_lang("これは日本語のテキストです。プロジェクトの背景を説明します。"),
            Some("ja")
        );
        assert_eq!(
            detect_lang("이것은 한국어 문서입니다. 프로젝트 배경을 설명합니다."),
            Some("ko")
        );
        assert_eq!(detect_lang("Plain English text only."), None);
    }

    #[test]
    fn icons_skip_headings_that_have_one() {
        assert_eq!(icon_for("0. 執行摘要（重點結論）"), Some("summary"));
        assert_eq!(icon_for("Performance Tuning"), Some("perf"));
        assert!(starts_with_emoji("✅ 已完成"));
        assert!(!starts_with_emoji("執行摘要"));
    }
}
