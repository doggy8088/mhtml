use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::Deserialize;

/// 頁面色彩主題。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
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
///
/// Defaults can be set in a config file (see --print-config-path); command-line
/// flags always take precedence over it.
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
  mhtml --init-config                   Create a commented config file

CUSTOM STYLING:
  The built-in style is driven by CSS custom properties (--mhtml-bg,
  --mhtml-fg, --mhtml-accent, --mhtml-max-width, ...). Override them in a file
  passed with --css, or use --accent / --width / --font / --font-size.

CONFIG FILE:
  Settings are read from config.toml in your OS config folder, or from the path
  in --config / $MHTML_CONFIG. Precedence: built-in defaults < config file <
  command-line flags."
)]
pub struct Cli {
    /// Markdown file to convert ('-' reads from stdin)
    #[arg(required_unless_present_any = ["print_config_path", "init_config"])]
    pub input: Option<String>,

    /// HTML file to write ('-' writes to stdout). Omit to preview in a browser
    pub output: Option<PathBuf>,

    /// Color theme [default: auto]
    #[arg(short, long, value_enum)]
    pub theme: Option<Theme>,

    /// Extra stylesheet (file path or http(s) URL); may be repeated.
    /// Files are inlined, URLs are linked. Applied after the config file's CSS
    #[arg(short, long, value_name = "FILE|URL")]
    pub css: Vec<String>,

    /// Skip the built-in stylesheet (only your CSS is used)
    #[arg(long, overrides_with = "default_style")]
    pub no_default_style: bool,

    /// Use the built-in stylesheet (to undo `default_style = false` in the config)
    #[arg(long, hide = true, overrides_with = "no_default_style")]
    pub default_style: bool,

    /// Page title (default: front matter title, first heading, or file name)
    #[arg(long)]
    pub title: Option<String>,

    /// Value of the <html lang> attribute [default: en]
    #[arg(long, value_name = "LANG")]
    pub lang: Option<String>,

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
    #[arg(long, overrides_with = "no_toc")]
    pub toc: bool,

    /// Do not insert a table of contents (to undo `toc = true` in the config)
    #[arg(long, hide = true, overrides_with = "toc")]
    pub no_toc: bool,

    /// Heading shown above the table of contents [default: Contents]
    #[arg(long, value_name = "TEXT")]
    pub toc_title: Option<String>,

    /// Disable syntax highlighting of fenced code blocks
    #[arg(long, overrides_with = "highlight")]
    pub no_highlight: bool,

    /// Enable syntax highlighting (to undo `highlight = false` in the config)
    #[arg(long, hide = true, overrides_with = "no_highlight")]
    pub highlight: bool,

    /// Disable $...$ / $$...$$ math rendering
    #[arg(long, overrides_with = "math")]
    pub no_math: bool,

    /// Enable math rendering (to undo `math = false` in the config)
    #[arg(long, hide = true, overrides_with = "no_math")]
    pub math: bool,

    /// Turn on smart punctuation (curly quotes, en/em dashes, ellipses)
    #[arg(long, overrides_with = "no_smart_punctuation")]
    pub smart_punctuation: bool,

    /// Turn off smart punctuation (to undo the config setting)
    #[arg(long, hide = true, overrides_with = "smart_punctuation")]
    pub no_smart_punctuation: bool,

    /// Keep raw HTML in the Markdown as-is (scripts included). By default the
    /// output is sanitized, which is the safe choice for untrusted files
    #[arg(long, overrides_with = "no_unsafe_html")]
    pub unsafe_html: bool,

    /// Sanitize raw HTML (to undo `unsafe_html = true` in the config)
    #[arg(long, hide = true, overrides_with = "unsafe_html")]
    pub no_unsafe_html: bool,

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

    /// Read settings from this config file instead of the default location
    /// (also: $MHTML_CONFIG)
    #[arg(long, value_name = "FILE", conflicts_with = "no_config")]
    pub config: Option<PathBuf>,

    /// Ignore the config file for this run
    #[arg(long)]
    pub no_config: bool,

    /// Print the default config file location and exit
    #[arg(long)]
    pub print_config_path: bool,

    /// Create a commented config file at the default location and exit
    #[arg(long)]
    pub init_config: bool,
}

/// Resolves a `--x` / `--no-x` flag pair: whichever was given wins, `None` when
/// neither was.
pub fn toggle(on: bool, off: bool) -> Option<bool> {
    match (on, off) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    }
}
