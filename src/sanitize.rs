//! HTML sanitizing for the rendered Markdown.
//!
//! Markdown allows raw HTML, so a hostile `.md` file could carry scripts. The
//! output is cleaned with an allow-list unless `--unsafe-html` is given.

use std::borrow::Cow;

use ammonia::Builder;

/// Presentation MathML elements produced by the math renderer. Elements that
/// can script or embed foreign content (`maction`, `mglyph`, `annotation-xml`,
/// `semantics`) are deliberately left out.
const MATHML_TAGS: &[&str] = &[
    "math", "mrow", "mi", "mn", "mo", "ms", "mtext", "mspace", "msup", "msub", "msubsup", "mfrac",
    "msqrt", "mroot", "mover", "munder", "munderover", "mtable", "mtr", "mtd", "mstyle", "mpadded",
    "menclose", "merror", "mphantom", "mmultiscripts", "mprescripts", "none",
];

const MATHML_ATTRS: &[&str] = &[
    "xmlns", "display", "mathvariant", "stretchy", "fence", "separator", "form", "accent",
    "accentunder", "displaystyle", "scriptlevel", "linethickness", "lspace", "rspace", "minsize",
    "maxsize", "mathsize", "movablelimits", "largeop", "symmetric", "width", "height", "depth",
    "columnalign", "rowalign", "columnspacing", "rowspacing", "columnlines", "rowlines", "frame",
    "notation", "columnspan", "rowspan",
];

pub fn clean(html: &str) -> String {
    let mut builder = Builder::default();

    // Task list checkboxes and a few structural tags the defaults leave out.
    builder.add_tags(["input", "section"]);
    builder.add_tags(MATHML_TAGS.iter().copied());
    for tag in MATHML_TAGS {
        builder.add_tag_attributes(tag, MATHML_ATTRS.iter().copied());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mathml_survives() {
        let html = "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\"><mfrac><mi>a</mi><mn>2</mn></mfrac></math>";
        let out = clean(html);
        assert!(out.contains("<mfrac>"), "{out}");
        assert!(out.contains("display=\"block\""), "{out}");
    }

    #[test]
    fn dangerous_mathml_is_removed() {
        let out = clean("<math><maction actiontype=\"statusline\" onclick=\"alert(1)\"><mi>x</mi></maction></math>");
        assert!(!out.contains("maction"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
    }

    #[test]
    fn disallowed_attributes_and_inputs_are_dropped() {
        let out = clean("<input type=\"text\" name=\"x\"><td style=\"background:url(x)\">c</td>");
        assert!(!out.contains("name="), "{out}");
        assert!(!out.contains("url("), "{out}");
    }
}
