use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// 頁面色彩主題。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    /// Follow the operating system / browser setting (prefers-color-scheme).
    Auto,
    /// Always use the light theme.
    Light,
    /// Always use the dark theme.
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Auto => "auto",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

/// Convert Markdown to a beautiful, self-contained HTML page.
///
/// With only INPUT, the page is written to a temporary file and opened in your
/// default browser for a quick preview. With OUTPUT, the HTML file is written
/// there instead.
#[derive(Debug, Parser)]
#[command(
    name = "mhtml",
    version,
    about,
    after_help = "\
EXAMPLES:
  mhtml README.md                       Preview in the default browser
  mhtml README.md README.html           Write a standalone HTML file
  mhtml README.md out.html --theme dark Dark theme
  mhtml doc.md out.html --css my.css    Add your own stylesheet
  cat doc.md | mhtml - -                Read from stdin, write to stdout

CUSTOM STYLING:
  The built-in style is driven by CSS custom properties (--mhtml-bg,
  --mhtml-fg, --mhtml-accent, --mhtml-max-width, ...). Override them in a file
  passed with --css, or use --accent / --width / --font / --font-size."
)]
pub struct Cli {
    /// Markdown file to convert ('-' reads from stdin)
    pub input: String,

    /// HTML file to write ('-' writes to stdout). Omit to preview in a browser
    pub output: Option<PathBuf>,

    /// Color theme
    #[arg(short, long, value_enum, default_value_t = Theme::Auto)]
    pub theme: Theme,

    /// Extra stylesheet (file path or http(s) URL); may be repeated.
    /// Files are inlined, URLs are linked
    #[arg(short, long, value_name = "FILE|URL")]
    pub css: Vec<String>,

    /// Skip the built-in stylesheet (only your --css is used)
    #[arg(long)]
    pub no_default_style: bool,

    /// Page title (default: front matter title, first heading, or file name)
    #[arg(long)]
    pub title: Option<String>,

    /// Value of the <html lang> attribute
    #[arg(long, default_value = "en", value_name = "LANG")]
    pub lang: String,

    /// Max width of the content column, any CSS length (e.g. 960px, 80ch)
    #[arg(long, value_name = "LENGTH")]
    pub width: Option<String>,

    /// Base font size, any CSS length (e.g. 18px)
    #[arg(long, value_name = "LENGTH")]
    pub font_size: Option<String>,

    /// Body font-family, any CSS font stack (e.g. "Georgia, serif")
    #[arg(long, value_name = "FONT")]
    pub font: Option<String>,

    /// Accent color for links and highlights, any CSS color (e.g. "#e11d48")
    #[arg(long, value_name = "COLOR")]
    pub accent: Option<String>,

    /// Insert a table of contents before the content
    #[arg(long)]
    pub toc: bool,

    /// Heading shown above the table of contents
    #[arg(long, value_name = "TEXT")]
    pub toc_title: Option<String>,

    /// Disable syntax highlighting of fenced code blocks
    #[arg(long)]
    pub no_highlight: bool,

    /// Disable $...$ / $$...$$ math rendering
    #[arg(long)]
    pub no_math: bool,

    /// Turn on smart punctuation (curly quotes, en/em dashes, ellipses)
    #[arg(long)]
    pub smart_punctuation: bool,

    /// Keep raw HTML in the Markdown as-is (scripts included). By default the
    /// output is sanitized, which is the safe choice for untrusted files
    #[arg(long)]
    pub unsafe_html: bool,

    /// <base href> for relative links and images. In preview mode it defaults
    /// to the Markdown file's folder
    #[arg(long, value_name = "URL")]
    pub base: Option<String>,

    /// Open the result in the browser even when OUTPUT is given
    #[arg(long, conflicts_with = "no_open")]
    pub open: bool,

    /// Do not open a browser in preview mode (just print the temp file path)
    #[arg(long)]
    pub no_open: bool,
}
