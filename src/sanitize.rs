//! HTML sanitizing for the rendered Markdown.
//!
//! Markdown allows raw HTML, so a hostile `.md` file could carry scripts. The
//! output is cleaned with an allow-list unless `--unsafe-html` is given.

use std::borrow::Cow;

use ammonia::Builder;

pub fn clean(html: &str) -> String {
    let mut builder = Builder::default();

    // Task list checkboxes and a few structural tags the defaults leave out.
    builder.add_tags(["input", "section"]);
    builder.add_generic_attributes(["id", "class", "align"]);
    builder.add_tag_attributes("input", ["type", "checked", "disabled"]);
    // Table column alignment is emitted as an inline style; see the filter.
    builder.add_tag_attributes("th", ["style"]);
    builder.add_tag_attributes("td", ["style"]);

    builder.attribute_filter(|element, attribute, value| match (element, attribute) {
        ("th" | "td", "style") => safe_text_align(value).map(Cow::Borrowed),
        ("input", "type") => (value == "checkbox").then_some(Cow::Borrowed(value)),
        _ => Some(Cow::Borrowed(value)),
    });

    builder.clean(html).to_string()
}

/// Only `text-align: left|center|right` may survive in a `style` attribute.
fn safe_text_align(value: &str) -> Option<&'static str> {
    let v = value.trim().trim_end_matches(';');
    match v.strip_prefix("text-align:")?.trim() {
        "left" => Some("text-align: left"),
        "center" => Some("text-align: center"),
        "right" => Some("text-align: right"),
        _ => None,
    }
}
