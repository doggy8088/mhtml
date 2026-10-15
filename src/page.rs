//! Wraps the rendered body into a complete HTML document.

use crate::cli::Theme;
use crate::style::{self, CssSource};

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

        format!(
            "<!doctype html>\n<html lang=\"{lang}\" data-theme=\"{theme}\">\n<head>\n{head}</head>\n\
             <body>\n<main class=\"markdown-body\">\n{body}</main>\n{script}</body>\n</html>\n",
            lang = escape(self.lang),
            theme = self.theme.as_str(),
            body = self.body,
            script = if self.base.is_some() { ANCHOR_FIX } else { "" },
        )
    }
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
}
