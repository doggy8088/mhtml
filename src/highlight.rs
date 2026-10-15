//! Syntax highlighting for fenced code blocks.
//!
//! Tokens are emitted as `<span class="hl-…">` (no inline colors), and the
//! matching CSS for a light and a dark palette is generated separately. That
//! lets the page switch palettes with `prefers-color-scheme` and keeps the HTML
//! small and themeable.

use std::sync::LazyLock;

use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use two_face::theme::EmbeddedThemeName;

const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

// Loading the syntax definitions takes a few milliseconds, so only do it when a
// document actually contains a fenced code block with a language.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// Highlights `code` and returns the inner HTML for `<code>`, or `None` when the
/// language is unknown (the caller then keeps the plain block).
pub fn highlight(info: &str, code: &str) -> Option<String> {
    let syntax = find_syntax(info)?;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, &SYNTAXES, CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        generator
            .parse_html_for_line_which_includes_newline(line)
            .ok()?;
    }
    Some(generator.finalize())
}

/// The language is the first word of the info string; `rust,ignore`,
/// `js {1,3}` and `ts title="x"` all work.
pub fn language(info: &str) -> &str {
    info.split(|c: char| c.is_whitespace() || c == ',' || c == '{')
        .next()
        .unwrap_or("")
}

fn find_syntax(info: &str) -> Option<&'static SyntaxReference> {
    let lang = language(info).to_lowercase();
    if lang.is_empty() {
        return None;
    }
    let alias = match lang.as_str() {
        "shell" | "zsh" | "console" | "shell-session" => "sh",
        "csharp" => "cs",
        "jsonc" | "json5" => "json",
        "text" | "txt" | "plain" | "plaintext" => return None,
        other => other,
    };
    SYNTAXES
        .find_syntax_by_token(alias)
        .or_else(|| SYNTAXES.find_syntax_by_extension(alias))
}

/// Stylesheet for the light palette.
pub fn css_light() -> String {
    palette_css(EmbeddedThemeName::Github)
}

/// Stylesheet for the dark palette.
pub fn css_dark() -> String {
    palette_css(EmbeddedThemeName::OneHalfDark)
}

fn palette_css(name: EmbeddedThemeName) -> String {
    let themes = two_face::theme::extra();
    let css = css_for_theme_with_class_style(themes.get(name), CLASS_STYLE).unwrap_or_default();
    // Drop the generated header comment and the `.hl-code` base rule: the page's
    // own code block colors (--mhtml-code-bg / --mhtml-code-fg) are used instead.
    let body = css.split_once("*/").map_or(css.as_str(), |(_, rest)| rest);
    body.split_once("\n}\n")
        .map_or(String::new(), |(_, rest)| rest.trim().to_string())
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_known_languages() {
        let html = highlight("rust", "fn main() { let x = 1; }\n").unwrap();
        assert!(html.contains("<span class=\"hl-"), "{html}");
        assert!(html.contains("main"));
    }

    #[test]
    fn escapes_html_in_code() {
        let html = highlight("html", "<script>alert(1)</script>\n").unwrap();
        assert!(!html.contains("<script>"), "{html}");
    }

    #[test]
    fn unknown_or_plain_languages_are_skipped() {
        assert!(highlight("definitely-not-a-language", "x").is_none());
        assert!(highlight("text", "x").is_none());
        assert!(highlight("", "x").is_none());
    }

    #[test]
    fn common_aliases_resolve() {
        for lang in [
            "js", "ts", "tsx", "toml", "yaml", "sh", "bash", "csharp", "py", "json", "md",
        ] {
            assert!(find_syntax(lang).is_some(), "{lang} not found");
        }
    }

    #[test]
    fn language_is_first_word_of_info_string() {
        assert_eq!(language("rust,ignore"), "rust");
        assert_eq!(language("js {1,3}"), "js");
        assert_eq!(language(""), "");
    }

    #[test]
    fn palettes_are_distinct_and_scoped_to_hl_classes() {
        let (l, d) = (css_light(), css_dark());
        assert_ne!(l, d);
        assert!(l.contains(".hl-"));
        assert!(!l.contains(".hl-code {"));
    }
}
