//! Wraps the rendered body into a complete HTML document.

pub struct Page<'a> {
    pub title: &'a str,
    pub lang: &'a str,
    pub body: &'a str,
}

impl Page<'_> {
    pub fn to_html(&self) -> String {
        format!(
            "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>{title}</title>\n</head>\n<body>\n<main class=\"markdown-body\">\n{body}</main>\n</body>\n</html>\n",
            lang = escape(self.lang),
            title = escape(self.title),
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
