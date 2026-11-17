//! The high-level API: Markdown in, one complete HTML page out.
//!
//! [`to_html`] wires the stages together — render the body, describe the
//! document, then wrap it in a self-contained page. The lower-level pieces stay
//! public ([`crate::render`], [`crate::page`], [`crate::style`]) for callers that
//! want to supply their own body or head.

use crate::enhance;
use crate::page::Page;
use crate::render::{self, RenderOptions, Warning};
use crate::style::{self, CssSource, InvalidCssValue};
use crate::theme::Theme;

/// Everything [`to_html`] needs.
///
/// [`Options::default`] matches what the `mhtml` command does with no flags and
/// no config file: adaptive presentation, syntax highlighting, math and Mermaid
/// on, sanitizing on, output self-contained.
#[derive(Debug, Clone)]
pub struct Options {
    /// Page title, and the banner above a document that has no H1 of its own.
    /// `None` uses the front matter title or the first heading.
    pub title: Option<String>,

    /// Last resort for the title when neither this nor the document has one.
    /// Defaults to `"Markdown"`.
    pub fallback_title: Option<String>,

    /// Value of `<html lang>`. `None` detects CJK text (when `enhance` is on)
    /// and otherwise uses `"en"`.
    pub lang: Option<String>,

    /// Color theme. Defaults to [`Theme::Auto`].
    pub theme: Theme,

    /// `<base href>` for relative links and images. Leave `None` when the page
    /// is written next to the Markdown it came from.
    pub base: Option<String>,

    /// Include the built-in stylesheets. Turn it off to bring your own CSS.
    pub default_style: bool,

    /// Adaptive presentation: layout by document shape, cards, property grids,
    /// interactive tables, side outline. Needs `default_style`.
    pub enhance: bool,

    /// Accent color shortcut for links and highlights (any CSS color).
    pub accent: Option<String>,
    /// Content column width shortcut (any CSS length, e.g. `"80ch"`).
    pub width: Option<String>,
    /// Body font-family shortcut (any CSS font stack).
    pub font: Option<String>,
    /// Base font size shortcut (any CSS length, e.g. `"18px"`).
    pub font_size: Option<String>,

    /// Ready-made `:root { ... }` block for advanced overrides. When set, it is
    /// used as-is and `accent`/`width`/`font`/`font_size` are ignored.
    pub css_overrides: Option<String>,

    /// Stylesheets, emitted last so they win the cascade.
    pub extra_css: Vec<CssSource>,

    /// Insert a table of contents before the content.
    pub toc: bool,
    /// Heading shown above the table of contents.
    pub toc_title: String,

    /// Syntax highlighting of fenced code blocks.
    pub highlight: bool,
    /// `$...$` / `$$...$$` math rendered to MathML (offline).
    pub math: bool,
    /// `mermaid` code fences rendered with the embedded renderer (offline).
    pub mermaid: bool,
    /// Curly quotes, en/em dashes, ellipses.
    pub smart_punctuation: bool,
    /// Keep raw HTML from the Markdown as-is, scripts included.
    pub unsafe_html: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            title: None,
            fallback_title: None,
            lang: None,
            theme: Theme::Auto,
            base: None,
            default_style: true,
            enhance: true,
            accent: None,
            width: None,
            font: None,
            font_size: None,
            css_overrides: None,
            extra_css: Vec::new(),
            toc: false,
            toc_title: "Contents".to_string(),
            highlight: true,
            math: true,
            mermaid: true,
            smart_punctuation: false,
            unsafe_html: false,
        }
    }
}

impl Options {
    /// Default options, ready to customize.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [`Options::title`].
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets [`Options::lang`].
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }

    /// Sets [`Options::theme`].
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Sets [`Options::base`].
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }

    /// Adds one stylesheet.
    pub fn with_css(mut self, css: CssSource) -> Self {
        self.extra_css.push(css);
        self
    }

    /// Turns on the table of contents with the given heading.
    pub fn with_toc(mut self, title: impl Into<String>) -> Self {
        self.toc = true;
        self.toc_title = title.into();
        self
    }

    /// Sets [`Options::accent`].
    pub fn with_accent(mut self, accent: impl Into<String>) -> Self {
        self.accent = Some(accent.into());
        self
    }
}

/// A finished page.
#[derive(Debug, Clone)]
pub struct Document {
    /// The complete `<!doctype html>` page, ready to write to a file.
    pub html: String,
    /// The sanitized `<body>` content on its own, for callers that build their
    /// own page around it.
    pub body: String,
    /// The title that was used, after front matter, heading and fallback.
    pub title: String,
    /// The `<html lang>` value that was used.
    pub lang: String,
    /// Emphasis markers that CommonMark — and therefore GitHub — leaves literal.
    pub warnings: Vec<Warning>,
    /// At least one code block was highlighted.
    pub uses_highlight: bool,
    /// At least one Mermaid diagram is on the page.
    pub uses_mermaid: bool,
}

impl Document {
    /// True when [`Document::warnings`] is not empty.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.html)
    }
}

/// Converts `markdown` into a complete, self-contained HTML page.
///
/// The only way this fails is a styling shortcut (`accent`, `width`, `font`,
/// `font_size`) whose value could escape its CSS declaration.
///
/// ```
/// let doc = mhtml::to_html("# Hello\n\nworld", &mhtml::Options::default()).unwrap();
/// assert!(doc.html.starts_with("<!doctype html>"));
/// assert_eq!(doc.title, "Hello");
/// ```
pub fn to_html(markdown: &str, options: &Options) -> Result<Document, InvalidCssValue> {
    // Built here rather than taken from the caller so that the shortcuts are
    // validated and escaped in one place.
    let overrides = match &options.css_overrides {
        Some(css) => Some(css.clone()),
        None => style::overrides_css(
            options.accent.as_deref(),
            options.width.as_deref(),
            options.font.as_deref(),
            options.font_size.as_deref(),
        )?,
    };

    let rendered = render::render(
        markdown,
        &RenderOptions {
            smart_punctuation: options.smart_punctuation,
            unsafe_html: options.unsafe_html,
            highlight: options.highlight,
            math: options.math,
            mermaid: options.mermaid,
            toc_title: options.toc.then(|| options.toc_title.clone()),
            // Adaptive presentation is CSS and a script, so it needs the
            // built-in stylesheet to describe.
            enhance: options.enhance && options.default_style,
        },
    );

    let lang = options
        .lang
        .clone()
        .or_else(|| {
            options
                .enhance
                .then(|| enhance::detect_lang(markdown))
                .flatten()
                .map(String::from)
        })
        .unwrap_or_else(|| "en".into());
    let main_class = rendered.profile.classes.join(" ");
    let title = options
        .title
        .clone()
        .or_else(|| rendered.title.clone())
        .or_else(|| options.fallback_title.clone())
        .unwrap_or_else(|| "Markdown".into());

    let html = Page {
        title: &title,
        lang: &lang,
        body: &rendered.body,
        theme: options.theme,
        base: options.base.as_deref(),
        default_style: options.default_style,
        enhance: options.enhance,
        main_class: &main_class,
        outline: rendered.outline.as_deref(),
        interactive: rendered.profile.interactive,
        title_banner: !rendered.has_h1 && rendered.profile.classes.contains(&"doc-prose"),
        highlight: rendered.uses_highlight,
        mermaid: rendered.uses_mermaid,
        overrides,
        extra_css: &options.extra_css,
    }
    .to_html();

    Ok(Document {
        html,
        body: rendered.body,
        title,
        lang,
        warnings: rendered.warnings,
        uses_highlight: rendered.uses_highlight,
        uses_mermaid: rendered.uses_mermaid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_produce_a_self_contained_page() {
        let doc = to_html("# Title\n\ntext", &Options::default()).unwrap();
        assert!(doc.html.starts_with("<!doctype html>"), "{}", doc.html);
        assert!(doc.html.trim_end().ends_with("</html>"));
        assert!(doc.html.contains("<title>Title</title>"));
        assert!(doc.html.contains("id=\"mhtml-base\""), "built-in CSS");
        // Nothing is fetched from the network.
        assert!(!doc.html.contains("<link rel=\"stylesheet\" href=\"http"));
    }

    #[test]
    fn title_precedence_is_option_then_document_then_fallback() {
        let opts = Options::default().with_title("From options");
        assert_eq!(to_html("# Heading", &opts).unwrap().title, "From options");

        let opts = Options {
            fallback_title: Some("From caller".into()),
            ..Options::default()
        };
        assert_eq!(to_html("# Heading", &opts).unwrap().title, "Heading");
        assert_eq!(to_html("no heading", &opts).unwrap().title, "From caller");
        assert_eq!(
            to_html("no heading", &Options::default()).unwrap().title,
            "Markdown"
        );
    }

    #[test]
    fn theme_and_lang_reach_the_html_element() {
        let opts = Options::default()
            .with_theme(Theme::Dark)
            .with_lang("zh-TW");
        let doc = to_html("# 標題", &opts).unwrap();
        assert!(
            doc.html
                .contains(r#"<html lang="zh-TW" data-theme="dark">"#),
            "{}",
            doc.html
        );
    }

    #[test]
    fn cjk_language_is_detected_when_enhance_is_on() {
        let doc = to_html(
            "# 繁體中文標題\n\n這是一段用來偵測語言的文字，長度足夠。",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(doc.lang, "zh-Hant");

        let opts = Options {
            enhance: false,
            ..Options::default()
        };
        assert_eq!(to_html("# 繁體中文標題", &opts).unwrap().lang, "en");
    }

    #[test]
    fn toc_is_opt_in() {
        let plain = to_html("# A", &Options::default()).unwrap();
        assert!(!plain.html.contains("<nav"), "{}", plain.html);
        let doc = to_html("# A\n\n## B", &Options::default().with_toc("目錄")).unwrap();
        assert!(doc.html.contains("目錄"), "{}", doc.html);
    }

    #[test]
    fn raw_html_is_sanitized_unless_asked_otherwise() {
        let md = "<script>alert(1)</script>\n\nhello";
        let safe = to_html(md, &Options::default()).unwrap();
        assert!(!safe.html.contains("<script>alert(1)"), "{}", safe.html);

        let opts = Options {
            unsafe_html: true,
            ..Options::default()
        };
        assert!(
            to_html(md, &opts)
                .unwrap()
                .html
                .contains("<script>alert(1)")
        );
    }

    #[test]
    fn invalid_shortcut_values_are_rejected() {
        let opts = Options {
            accent: Some("red; } body { display:none".into()),
            ..Options::default()
        };
        assert!(to_html("x", &opts).is_err());

        let opts = Options {
            width: Some("</style>".into()),
            ..Options::default()
        };
        assert!(to_html("x", &opts).is_err());
    }

    #[test]
    fn shortcuts_become_custom_properties_and_overrides_win() {
        let opts = Options::default().with_accent("#123456");
        let html = to_html("x", &opts).unwrap().html;
        assert!(html.contains("--mhtml-accent: #123456;"), "{html}");

        let opts = Options {
            accent: Some("#123456".into()),
            css_overrides: Some(":root { --mhtml-accent: blue; }".into()),
            ..Options::default()
        };
        let html = to_html("x", &opts).unwrap().html;
        assert!(html.contains("--mhtml-accent: blue;"));
        assert!(!html.contains("#123456"), "the raw block wins");
    }

    #[test]
    fn extra_css_is_emitted_after_the_built_in_stylesheets() {
        let opts = Options::default().with_css(CssSource::Inline {
            name: "mine.css".into(),
            css: "h1 { color: red }".into(),
        });
        let html = to_html("# A", &opts).unwrap().html;
        let built_in = html.find("id=\"mhtml-base\"").unwrap();
        let mine = html.find("h1 { color: red }").unwrap();
        assert!(built_in < mine, "{html}");
    }

    #[test]
    fn no_default_style_leaves_only_the_callers_css() {
        let opts = Options {
            default_style: false,
            ..Options::default()
        };
        let html = to_html("# A", &opts).unwrap().html;
        assert!(!html.contains("id=\"mhtml-base\""));
        assert!(!html.contains("id=\"mhtml-enhance\""));
    }

    #[test]
    fn emphasis_warnings_are_reported() {
        let doc = to_html("**unpaired marker", &Options::default()).unwrap();
        assert!(doc.has_warnings(), "{:?}", doc.warnings);
        let clean = to_html("**paired**", &Options::default()).unwrap();
        assert!(!clean.has_warnings());
    }

    #[test]
    fn body_is_the_content_without_the_page() {
        let doc = to_html("# A\n\npara", &Options::default()).unwrap();
        assert!(doc.body.contains("<h1"), "{}", doc.body);
        assert!(!doc.body.contains("<!doctype"), "{}", doc.body);
        assert!(doc.html.contains(&doc.body));
    }
}
