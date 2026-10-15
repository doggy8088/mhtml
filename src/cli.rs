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

// The help texts live in assets/help-*.txt so they are easy to edit and review;
// the tests below keep them in sync with the real flags, config keys and CSS.
#[derive(Debug, Parser)]
#[command(
    name = "mhtml",
    version,
    about = "Convert Markdown to a beautiful, self-contained HTML page",
    long_about = include_str!("../assets/help-long-about.txt"),
    after_help = include_str!("../assets/help-after-short.txt"),
    after_long_help = include_str!("../assets/help-after-long.txt"),
    override_usage = "mhtml [OPTIONS] <INPUT> [OUTPUT]\n       mhtml --init-config | --print-config-path",
    help_template = "{name} {version}\n{usage-heading} {usage}\n\n{about-with-newline}\n{all-args}\n{after-help}"
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

#[cfg(test)]
mod help_tests {
    //! The help text is documentation for people *and* AI agents, so it must not
    //! drift from the real flags, config keys, CSS variables and behavior.

    use std::collections::BTreeSet;

    use clap::CommandFactory;

    use super::*;

    const LONG_ABOUT: &str = include_str!("../assets/help-long-about.txt");
    const AFTER_SHORT: &str = include_str!("../assets/help-after-short.txt");
    const AFTER_LONG: &str = include_str!("../assets/help-after-long.txt");
    const STYLE_CSS: &str = concat!(
        include_str!("../assets/style.css"),
        include_str!("../assets/theme-light.css"),
        include_str!("../assets/theme-dark.css")
    );
    const CONFIG_TEMPLATE: &str = include_str!("../assets/config.toml");

    fn all_help() -> String {
        format!("{LONG_ABOUT}\n{AFTER_SHORT}\n{AFTER_LONG}")
    }

    /// Words that start with `prefix` and continue with `[a-z0-9-]`.
    fn tokens(text: &str, prefix: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(pos) = rest.find(prefix) {
            let tail = &rest[pos..];
            let len = tail
                .char_indices()
                .find(|&(i, c)| {
                    i >= prefix.len() && !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                })
                .map_or(tail.len(), |(i, _)| i);
            found.insert(tail[..len].trim_end_matches('-').to_string());
            rest = &tail[len.max(prefix.len())..];
        }
        found
    }

    /// Splits a command line like a shell would for plain words and "double quotes".
    fn split_words(line: &str) -> Vec<String> {
        let (mut words, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
        for c in line.chars() {
            match c {
                '"' => {
                    quoted = !quoted;
                    any = true;
                }
                c if c.is_whitespace() && !quoted => {
                    if any || !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                        any = false;
                    }
                }
                c => cur.push(c),
            }
        }
        if any || !cur.is_empty() {
            words.push(cur);
        }
        words
    }

    #[test]
    fn help_lines_fit_the_wrap_width() {
        // clap re-wraps lines wider than 100 columns without hanging indents, which
        // wrecks the layout of the examples and tables.
        for (name, text) in [
            ("long-about", LONG_ABOUT),
            ("after-short", AFTER_SHORT),
            ("after-long", AFTER_LONG),
        ] {
            for (n, line) in text.lines().enumerate() {
                assert!(
                    line.chars().count() <= 98,
                    "{name}:{} is too wide: {line}",
                    n + 1
                );
            }
        }
    }

    #[test]
    fn every_example_command_parses() {
        let mut checked = 0;
        for text in [LONG_ABOUT, AFTER_LONG] {
            for line in text.lines() {
                let Some(cmd) = line.trim_start().strip_prefix("$ mhtml ") else {
                    continue;
                };
                // Drop the trailing `   # comment`.
                let cmd = cmd.split("  #").next().unwrap().trim();
                let mut argv = vec!["mhtml".to_string()];
                argv.extend(split_words(cmd));
                Cli::try_parse_from(&argv)
                    .unwrap_or_else(|e| panic!("example does not parse: {line}\n{e}"));
                checked += 1;
            }
        }
        assert!(checked >= 20, "only {checked} examples were checked");
    }

    #[test]
    fn every_flag_in_the_help_exists() {
        let command = Cli::command();
        let real: BTreeSet<String> = command
            .get_arguments()
            .filter_map(|a| a.get_long().map(|l| format!("--{l}")))
            .chain(["--help".to_string(), "--version".to_string()])
            .collect();
        for flag in tokens(&all_help(), "--") {
            if flag.starts_with("--mhtml-") {
                continue; // CSS custom property, not a flag
            }
            assert!(real.contains(&flag), "help mentions unknown flag {flag}");
        }
    }

    #[test]
    fn every_user_facing_flag_is_documented() {
        let help = all_help();
        let command = Cli::command();
        for arg in command.get_arguments() {
            let Some(long) = arg.get_long() else { continue };
            if matches!(long, "help" | "version") {
                continue;
            }
            assert!(
                help.contains(&format!("--{long}")),
                "--{long} is missing from the guide"
            );
        }
    }

    #[test]
    fn css_variables_in_the_help_match_the_stylesheets() {
        let documented = tokens(AFTER_LONG, "--mhtml-");
        let defined: BTreeSet<String> = tokens(STYLE_CSS, "--mhtml-");
        for var in &documented {
            assert!(
                defined.contains(var),
                "help mentions {var}, which the CSS does not define"
            );
        }
        for var in &defined {
            assert!(
                documented.contains(var),
                "CSS defines {var}, which the help does not mention"
            );
        }
    }

    #[test]
    fn config_keys_in_the_help_match_the_template() {
        let keys: BTreeSet<&str> = CONFIG_TEMPLATE
            .lines()
            .filter_map(|l| l.strip_prefix("# "))
            .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim()))
            .filter(|k| k.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
            .collect();
        assert!(keys.len() >= 16, "template keys: {keys:?}");
        let keys_line: String = AFTER_LONG
            .lines()
            .skip_while(|l| !l.trim_start().starts_with("Keys:"))
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        for key in keys {
            assert!(
                keys_line.contains(key),
                "config key `{key}` is missing from the help"
            );
        }
    }

    #[test]
    fn the_guide_has_the_sections_agents_rely_on() {
        let help = all_help();
        for section in [
            "QUICK START",
            "RULES OF THUMB FOR AGENTS",
            "OUTPUT CONTRACT",
            "EXAMPLES",
            "STYLING HOOKS",
            "MARKDOWN SUPPORTED",
            "CONFIG FILE",
            "SECURITY",
            "TROUBLESHOOTING",
        ] {
            assert!(help.contains(section), "missing section {section}");
        }
    }
}
