//! Page color theme.

/// Page color theme.
///
/// `Auto` emits the light palette and lets the dark one take over through
/// `prefers-color-scheme`, so no JavaScript is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum, serde::Deserialize))]
#[cfg_attr(feature = "cli", serde(rename_all = "lowercase"))]
pub enum Theme {
    /// Follow the operating system / browser setting (prefers-color-scheme).
    #[default]
    Auto,
    /// Always use the light theme.
    Light,
    /// Always use the dark theme.
    Dark,
}

impl Theme {
    /// The value written to `<html data-theme="...">`.
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Auto => "auto",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}
