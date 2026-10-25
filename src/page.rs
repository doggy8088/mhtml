//! Wraps the rendered body into a complete HTML document.

use crate::cli::Theme;
use crate::style::{self, CssSource};

/// Ondine renderer, embedded only when the document has a Mermaid diagram.
const MERMAID_JS: &str = include_str!("../assets/ondine/ondine.js");
const MERMAID_BOOT: &str = include_str!("../assets/mermaid.js");
const MERMAID_WASM: &[u8] = include_bytes!("../assets/ondine/all.generated.wasm");

/// With `<base href>` set, `href="#id"` would resolve against the base URL and
/// leave the page. This keeps in-page anchors (TOC, footnotes) working.
const ANCHOR_FIX: &str = "<script>\n\
document.addEventListener('click', function (e) {\n\
  var a = e.target.closest && e.target.closest('a[href^=\"#\"]');\n\
  if (!a || e.defaultPrevented) return;\n\
  var id = decodeURIComponent(a.getAttribute('href').slice(1));\n\
  var el = id ? document.getElementById(id) : document.documentElement;\n\
  if (el) { e.preventDefault(); el.scrollIntoView(); }\n\
});\n\
</script>\n";

pub struct Page<'a> {
    pub title: &'a str,
    pub lang: &'a str,
    pub body: &'a str,
    pub theme: Theme,
    /// `<base href>` for relative links and images.
    pub base: Option<&'a str>,
    /// Include the built-in theme and stylesheet.
    pub default_style: bool,
    /// Include the syntax highlighting palette.
    pub highlight: bool,
    /// Embed the Mermaid renderer (the body contains `.mermaid-diagram` blocks).
    pub mermaid: bool,
    /// `:root { --mhtml-... }` block built from --accent/--width/...
    pub overrides: Option<String>,
    /// User stylesheets, emitted last so they win.
    pub extra_css: &'a [CssSource],
}

impl Page<'_> {
    pub fn to_html(&self) -> String {
        let mut head = String::new();
        head.push_str("<meta charset=\"utf-8\">\n");
        head.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        if let Some(base) = self.base {
            head.push_str(&format!("<base href=\"{}\">\n", escape(base)));
        }
        head.push_str(&format!("<title>{}</title>\n", escape(self.title)));

        if self.default_style {
            head.push_str(&format!(
                "<style id=\"mhtml-theme\">\n{}</style>\n",
                style::theme_css(self.theme, self.highlight)
            ));
            head.push_str(&format!(
                "<style id=\"mhtml-base\">\n{}</style>\n",
                style::BASE
            ));
            if let Some(overrides) = &self.overrides {
                head.push_str(&format!(
                    "<style id=\"mhtml-overrides\">\n{overrides}</style>\n"
                ));
            }
        }
        for css in self.extra_css {
            match css {
                CssSource::Link(url) => {
                    head.push_str(&format!(
                        "<link rel=\"stylesheet\" href=\"{}\">\n",
                        escape(url)
                    ));
                }
                CssSource::Inline { name, css } => {
                    head.push_str(&format!(
                        "<style data-source=\"{}\">\n{}\n</style>\n",
                        escape(name),
                        style::escape_style(css)
                    ));
                }
            }
        }

        let mut scripts = String::new();
        if self.base.is_some() {
            scripts.push_str(ANCHOR_FIX);
        }
        if self.mermaid {
            scripts
                .push_str("<script type=\"application/octet-stream\" id=\"mhtml-mermaid-wasm\">");
            scripts.push_str(&base64_encode(MERMAID_WASM));
            scripts.push_str("</script>\n");
            scripts.push_str("<script id=\"mhtml-mermaid-runtime\">\n");
            scripts.push_str(&escape_script(MERMAID_JS));
            scripts.push('\n');
            scripts.push_str(&escape_script(MERMAID_BOOT));
            scripts.push_str("</script>\n");
        }

        format!(
            "<!doctype html>\n<html lang=\"{lang}\" data-theme=\"{theme}\">\n<head>\n{head}</head>\n\
             <body>\n<main class=\"markdown-body\">\n{body}</main>\n{scripts}</body>\n</html>\n",
            lang = escape(self.lang),
            theme = self.theme.as_str(),
            body = self.body,
        )
    }
}

/// Standard base64 with padding, so the WASM can live in an inert `<script>`
/// element without any character that HTML would treat specially.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        out.push(ALPHABET[(chunk[0] >> 2) as usize] as char);
        out.push(ALPHABET[(((chunk[0] & 0x03) << 4) | (second >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Keeps embedded JavaScript from ending the `<script>` element early.
fn escape_script(js: &str) -> String {
    let mut out = String::with_capacity(js.len());
    let mut rest = js;
    while let Some(pos) = rest.to_ascii_lowercase().find("</script") {
        out.push_str(&rest[..pos]);
        out.push_str("<\\/script");
        rest = &rest[pos + "</script".len()..];
    }
    out.push_str(rest);
    out
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(base: Option<&str>) -> String {
        Page {
            title: "t",
            lang: "en",
            body: "<p>x</p>",
            theme: Theme::Auto,
            base,
            default_style: false,
            highlight: false,
            mermaid: false,
            overrides: None,
            extra_css: &[],
        }
        .to_html()
    }

    #[test]
    fn anchor_fix_only_with_base() {
        assert!(!page(None).contains("<script>"));
        let with = page(Some("file:///a/"));
        assert!(with.contains("<base href=\"file:///a/\">"));
        assert!(with.contains("scrollIntoView"));
    }

    #[test]
    fn title_is_escaped() {
        assert!(page(None).contains("<title>t</title>"));
        assert_eq!(
            escape("<a href=\"x\">&"),
            "&lt;a href=&quot;x&quot;&gt;&amp;"
        );
    }

    #[test]
    fn mermaid_renderer_is_embedded_only_when_asked() {
        let plain = page(None);
        assert!(!plain.contains("mhtml-mermaid-wasm"));
        assert!(!plain.contains("Ondine"));

        let with = Page {
            title: "t",
            lang: "en",
            body: "<div class=\"mermaid-diagram\"></div>",
            theme: Theme::Auto,
            base: None,
            default_style: false,
            highlight: false,
            mermaid: true,
            overrides: None,
            extra_css: &[],
        }
        .to_html();
        assert!(with.contains("id=\"mhtml-mermaid-wasm\""), "{with}");
        assert!(with.contains("Ondine.create"), "{with}");
        assert!(with.contains("mermaid-diagram"), "{with}");
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), expected);
        }
    }

    #[test]
    fn script_end_tags_are_neutralised() {
        let out = escape_script("a</SCRIPT><script>x()</script>");
        assert!(!out.to_ascii_lowercase().contains("</script"), "{out}");
    }
}
