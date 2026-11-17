//! Turn Markdown into a beautiful, **single-file, offline-friendly** HTML page.
//!
//! This crate is the engine behind the `mhtml` command line tool. The usual
//! entry point is [`to_html`], which returns the whole page as a string:
//!
//! ```
//! use mhtml::{CssSource, Options, Theme};
//!
//! let options = Options::default()
//!     .with_title("Release notes")
//!     .with_theme(Theme::Dark)
//!     .with_css(CssSource::Inline {
//!         name: "mine.css".into(),
//!         css: "h1 { letter-spacing: -0.02em }".into(),
//!     });
//!
//! let doc = mhtml::to_html("# v1.2.0\n\n- faster\n- smaller", &options).unwrap();
//! std::fs::write("notes.html", doc.html).unwrap();
//! # std::fs::remove_file("notes.html").unwrap();
//! ```
//!
//! Everything is inlined — stylesheets, syntax highlighting, MathML, the
//! Mermaid renderer — so the result needs no network and no sibling files.
//!
//! # Layers
//!
//! * [`to_html`] — Markdown → complete page. Start here.
//! * [`render::render`] — Markdown → sanitized `<body>` plus the profile the
//!   page needs (title, outline, warnings).
//! * [`page::Page`] — body + head settings → complete page, if you already have
//!   HTML from somewhere else.
//! * [`style`] and [`theme`] — the stylesheets and their CSS variables.
//!
//! # Features
//!
//! `cli` (on by default) also exposes the command line interface and the config
//! file support: [`cli`], [`config`] and [`preview`]. A library user who does
//! not need those can drop `clap`, `toml` and the browser-opening dependency:
//!
//! ```toml
//! mhtml = { git = "https://github.com/doggy8088/mhtml", default-features = false }
//! ```

pub mod document;
pub mod enhance;
pub mod highlight;
pub mod page;
pub mod render;
pub mod sanitize;
pub mod style;
pub mod theme;

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "cli")]
pub mod config;
#[cfg(feature = "cli")]
pub mod preview;

pub use document::{Document, Options, to_html};
pub use page::Page;
pub use render::{RenderOptions, Rendered, Warning};
pub use style::CssSource;
pub use theme::Theme;
