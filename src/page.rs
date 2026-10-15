//! Wraps the rendered body into a complete HTML document.

use crate::cli::Theme;
use crate::style::{self, CssSource};

pub struct Page<'a> {
    pub title: &'a str,
    pub lang: &'a str,
    pub body: &'a str,
    pub theme: Theme,
    /// `<base href>` for relative links and images.
    pub base: Option<&'a str>,
    /// Include the built-in theme and stylesheet.
    pub default_style: bool,
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
                style::theme_css(self.theme)
            ));
            head.push_str(&format!("<style id=\"mhtml-base\">\n{}</style>\n", style::BASE));
            if let Some(overrides) = &self.overrides {
                head.push_str(&format!("<style id=\"mhtml-overrides\">\n{overrides}</style>\n"));
            }
        }
        for css in self.extra_css {
            match css {
                CssSource::Link(url) => {
                    head.push_str(&format!("<link rel=\"stylesheet\" href=\"{}\">\n", escape(url)));
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
             <body>\n<main class=\"markdown-body\">\n{body}</main>\n</body>\n</html>\n",
            lang = escape(self.lang),
            theme = self.theme.as_str(),
            body = self.body,
        )
    }
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
