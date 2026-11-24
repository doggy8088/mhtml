//! HTML sanitizing for the rendered Markdown.
//!
//! Markdown allows raw HTML, so a hostile `.md` file could carry scripts. The
//! output is cleaned with an allow-list unless `--unsafe-html` is given.

use std::borrow::Cow;

use ammonia::{Builder, Url, UrlRelative, UrlRelativeEvaluate};

/// Presentation MathML elements produced by the math renderer. Elements that
/// can script or embed foreign content (`maction`, `mglyph`, `annotation-xml`,
/// `semantics`) are deliberately left out.
const MATHML_TAGS: &[&str] = &[
    "math",
    "mrow",
    "mi",
    "mn",
    "mo",
    "ms",
    "mtext",
    "mspace",
    "msup",
    "msub",
    "msubsup",
    "mfrac",
    "msqrt",
    "mroot",
    "mover",
    "munder",
    "munderover",
    "mtable",
    "mtr",
    "mtd",
    "mstyle",
    "mpadded",
    "menclose",
    "merror",
    "mphantom",
    "mmultiscripts",
    "mprescripts",
    "none",
];

const MATHML_ATTRS: &[&str] = &[
    "xmlns",
    "display",
    "mathvariant",
    "stretchy",
    "fence",
    "separator",
    "form",
    "accent",
    "accentunder",
    "displaystyle",
    "scriptlevel",
    "linethickness",
    "lspace",
    "rspace",
    "minsize",
    "maxsize",
    "mathsize",
    "movablelimits",
    "largeop",
    "symmetric",
    "width",
    "height",
    "depth",
    "columnalign",
    "rowalign",
    "columnspacing",
    "rowspacing",
    "columnlines",
    "rowlines",
    "frame",
    "notation",
    "columnspan",
    "rowspan",
];

pub fn clean(html: &str) -> String {
    clean_with_mode(html, false, None)
}

pub(crate) fn clean_content(html: &str, base: Option<&Url>) -> String {
    clean_with_mode(html, true, base)
}

fn clean_with_mode(html: &str, content_only: bool, base: Option<&Url>) -> String {
    let mut builder = Builder::default();

    // Task list checkboxes and a few structural tags the defaults leave out.
    builder.add_tags(["section"]);
    if content_only {
        if let Some(base) = base {
            // A document-wide <base> would also redirect heading and footnote links.
            builder.url_relative(UrlRelative::Custom(Box::new(ContentBase(base))));
        }
    } else {
        builder.add_tags(["input"]);
        builder.add_tag_attributes("input", ["type", "checked", "disabled"]);
    }
    builder.add_tags(MATHML_TAGS.iter().copied());
    for tag in MATHML_TAGS {
        builder.add_tag_attributes(tag, MATHML_ATTRS.iter().copied());
    }
    builder.add_tag_attributes("math", ["data-mhtml-latex"]);
    builder.add_generic_attributes(["id", "class", "align"]);
    // Table column alignment is emitted as an inline style; see the filter.
    builder.add_tag_attributes("th", ["style"]);
    builder.add_tag_attributes("td", ["style"]);

    // Inline images (`![](data:image/png;base64,...)`) need the `data:` scheme;
    // the filter below limits it to `<img src>` with an image media type.
    builder.add_url_schemes(["data"]);

    builder.attribute_filter(|element, attribute, value| match (element, attribute) {
        ("img", "src") if is_data_url(value) => {
            is_data_image(value).then_some(Cow::Borrowed(value))
        }
        (
            _,
            "href" | "xlink:href" | "src" | "action" | "data" | "formaction" | "ping" | "poster",
        ) if is_data_url(value) => None,
        ("th" | "td", "style") => safe_text_align(value).map(Cow::Borrowed),
        ("input", "type") => (value == "checkbox").then_some(Cow::Borrowed(value)),
        _ => Some(Cow::Borrowed(value)),
    });

    let html = builder.clean(html).to_string();
    if content_only {
        // The sanitized serialization escapes angle brackets in text and attributes.
        // Keep block boundaries while turning collapsible markup into visible content.
        html.replace("<details>", "<div>")
            .replace("<details ", "<div ")
            .replace("</details>", "</div>")
            .replace("<summary>", "<div>")
            .replace("<summary ", "<div ")
            .replace("</summary>", "</div>")
    } else {
        html
    }
}

struct ContentBase<'a>(&'a Url);

impl<'a> UrlRelativeEvaluate<'a> for ContentBase<'a> {
    fn evaluate<'url>(&self, url: &'url str) -> Option<Cow<'url, str>> {
        if url.trim_start().starts_with('#') {
            Some(Cow::Borrowed(url))
        } else {
            self.0.join(url).ok().map(|url| Cow::Owned(url.into()))
        }
    }
}

fn is_data_url(value: &str) -> bool {
    value
        .trim_start()
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
}

/// `data:image/...` only. An `<img>` never runs scripts, SVG included.
fn is_data_image(value: &str) -> bool {
    value
        .trim_start()
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:image/"))
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
        let out = clean(
            "<math><maction actiontype=\"statusline\" onclick=\"alert(1)\"><mi>x</mi></maction></math>",
        );
        assert!(!out.contains("maction"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
    }

    #[test]
    fn inline_data_images_survive() {
        let out = clean("<img src=\"data:image/jpeg;base64,/9j/4AAQ\" alt=\"shot\">");
        assert!(
            out.contains("src=\"data:image/jpeg;base64,/9j/4AAQ\""),
            "{out}"
        );
    }

    #[test]
    fn other_data_urls_are_dropped() {
        let out = clean(
            "<a href=\"data:text/html,<script>alert(1)</script>\">x</a><img src=\"data:text/html,hi\">",
        );
        assert!(!out.contains("data:"), "{out}");
    }

    #[test]
    fn disallowed_attributes_and_inputs_are_dropped() {
        let out = clean("<input type=\"text\" name=\"x\"><td style=\"background:url(x)\">c</td>");
        assert!(!out.contains("name="), "{out}");
        assert!(!out.contains("url("), "{out}");
    }
}
