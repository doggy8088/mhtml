//! Stylesheets: built-in themes, command-line overrides and user CSS.

use std::fs;

use anyhow::{Context, Result, bail};

use crate::cli::Theme;
use crate::highlight;

const LIGHT: &str = include_str!("../assets/theme-light.css");
const DARK: &str = include_str!("../assets/theme-dark.css");
pub const BASE: &str = include_str!("../assets/style.css");

/// Color variables for the chosen theme.
///
/// `auto` emits the light palette and lets the dark palette take over inside a
/// `prefers-color-scheme: dark` media query, so no JavaScript is needed.
/// With `highlight`, the matching syntax highlighting palette is appended.
pub fn theme_css(theme: Theme, highlight: bool) -> String {
    let (hl_light, hl_dark) = if highlight {
        (highlight::css_light(), highlight::css_dark())
    } else {
        (String::new(), String::new())
    };
    match theme {
        Theme::Light => format!("{LIGHT}\n{hl_light}"),
        Theme::Dark => format!("{DARK}\n{hl_dark}"),
        // The syntax palettes use selectors of different specificity, so the
        // light one must not be active at all in dark mode: wrap it in the
        // opposite media query instead of relying on source order.
        Theme::Auto => format!(
            "{LIGHT}\n@media not all and (prefers-color-scheme: dark) {{\n{hl_light}\n}}\n\
             @media (prefers-color-scheme: dark) {{\n{DARK}\n{hl_dark}\n}}\n"
        ),
    }
}

/// Turns the `--accent`, `--width`, `--font`, `--font-size` shortcuts into a
/// `:root { ... }` block that overrides the built-in variables.
pub fn overrides_css(
    accent: Option<&str>,
    width: Option<&str>,
    font: Option<&str>,
    font_size: Option<&str>,
) -> Result<Option<String>> {
    let mut decls = String::new();
    for (flag, var, value) in [
        ("accent", "--mhtml-accent", accent),
        ("width", "--mhtml-max-width", width),
        ("font", "--mhtml-font", font),
        ("font-size", "--mhtml-font-size", font_size),
    ] {
        if let Some(value) = value {
            check_css_value(flag, value)?;
            decls.push_str(&format!("  {var}: {};\n", value.trim()));
        }
    }
    Ok((!decls.is_empty()).then(|| format!(":root {{\n{decls}}}\n")))
}

/// Values are pasted into a `<style>` block, so refuse anything that could
/// close the declaration or the element.
fn check_css_value(flag: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() || value.contains(['{', '}', ';', '<', '>']) {
        bail!("invalid value for {flag}: {value:?} (expected a plain CSS value)");
    }
    Ok(())
}

/// A user supplied stylesheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssSource {
    /// A local file whose content is embedded in the page.
    Inline { name: String, css: String },
    /// A remote stylesheet referenced with `<link>`.
    Link(String),
}

impl CssSource {
    pub fn load(spec: &str) -> Result<Self> {
        if spec.starts_with("http://") || spec.starts_with("https://") {
            return Ok(CssSource::Link(spec.to_string()));
        }
        let css =
            fs::read_to_string(spec).with_context(|| format!("failed to read CSS file {spec}"))?;
        Ok(CssSource::Inline {
            name: spec.to_string(),
            css,
        })
    }
}

/// Keeps a stylesheet from ending its `<style>` element early.
pub fn escape_style(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(pos) = rest.to_ascii_lowercase().find("</style") {
        out.push_str(&rest[..pos]);
        out.push_str("<\\/style");
        rest = &rest[pos + "</style".len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_theme_contains_both_palettes() {
        let css = theme_css(Theme::Auto, false);
        assert!(css.contains("color-scheme: light"));
        assert!(css.contains("prefers-color-scheme: dark"));
        assert!(css.contains("color-scheme: dark"));
    }

    #[test]
    fn explicit_themes_have_single_palette() {
        assert!(!theme_css(Theme::Light, false).contains("prefers-color-scheme"));
        assert!(!theme_css(Theme::Dark, false).contains("color-scheme: light"));
    }

    #[test]
    fn highlight_palettes_follow_the_theme() {
        let auto = theme_css(Theme::Auto, true);
        assert!(auto.contains(".hl-"));
        // light palette only outside dark mode, dark palette only inside it
        let (before_dark, dark) = auto
            .split_once("@media (prefers-color-scheme: dark)")
            .unwrap();
        assert!(before_dark.contains("not all and (prefers-color-scheme: dark)"));
        assert!(before_dark.contains(".hl-") && dark.contains(".hl-"));
        assert!(auto.matches("  color-scheme: ").count() == 2);
        assert!(theme_css(Theme::Light, true).contains(".hl-"));
        assert!(!theme_css(Theme::Light, false).contains(".hl-"));
    }

    #[test]
    fn overrides_become_custom_properties() {
        let css = overrides_css(Some("#e11d48"), Some("70ch"), None, None)
            .unwrap()
            .unwrap();
        assert!(css.contains("--mhtml-accent: #e11d48;"));
        assert!(css.contains("--mhtml-max-width: 70ch;"));
        assert!(overrides_css(None, None, None, None).unwrap().is_none());
    }

    #[test]
    fn overrides_reject_injection() {
        assert!(overrides_css(Some("red; } body { display:none"), None, None, None).is_err());
        assert!(overrides_css(None, Some("</style><script>"), None, None).is_err());
    }

    #[test]
    fn style_end_tag_is_neutralised() {
        let out = escape_style("a{} </STYLE><script>x</script>");
        assert!(!out.to_ascii_lowercase().contains("</style"));
    }
}
