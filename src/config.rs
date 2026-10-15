//! Config file support.
//!
//! Settings live in `config.toml` inside the OS-specific config folder
//! (`~/Library/Application Support/mhtml` on macOS, `$XDG_CONFIG_HOME/mhtml` or
//! `~/.config/mhtml` on Linux, `%APPDATA%\mhtml` on Windows). Precedence is:
//! built-in defaults < config file < command-line flags.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::cli::{toggle, Cli, Theme};
use crate::style::{self, CssSource};

/// Commented-out template written by `--init-config`.
pub const TEMPLATE: &str = include_str!("../assets/config.toml");

/// Mirror of the config file. Every key is optional; unknown keys are rejected
/// so that a typo is reported instead of silently ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: Option<Theme>,
    pub lang: Option<String>,
    pub width: Option<String>,
    pub font_size: Option<String>,
    pub font: Option<String>,
    pub accent: Option<String>,
    pub default_style: Option<bool>,
    pub css: Vec<String>,
    pub custom_css: Option<String>,
    pub toc: Option<bool>,
    pub toc_title: Option<String>,
    pub highlight: Option<bool>,
    pub math: Option<bool>,
    pub smart_punctuation: Option<bool>,
    pub unsafe_html: Option<bool>,
    pub open_browser: Option<bool>,
}

/// A parsed config file plus where it came from.
#[derive(Debug, Default)]
pub struct Loaded {
    pub config: Config,
    pub path: Option<PathBuf>,
}

/// The effective settings after merging defaults, config file and flags.
#[derive(Debug)]
pub struct Settings {
    pub theme: Theme,
    pub lang: String,
    pub default_style: bool,
    pub overrides: Option<String>,
    pub extra_css: Vec<CssSource>,
    pub toc_title: Option<String>,
    pub highlight: bool,
    pub math: bool,
    pub smart_punctuation: bool,
    pub unsafe_html: bool,
    pub preview_open: bool,
}

/// Default config location for this OS.
pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("mhtml").join("config.toml"))
}

/// `--config`, then `$MHTML_CONFIG`; `true` in the tuple means "asked for
/// explicitly", so a missing file is an error rather than being skipped.
fn requested_path(cli: &Cli) -> Option<PathBuf> {
    cli.config
        .clone()
        .or_else(|| env::var_os("MHTML_CONFIG").filter(|v| !v.is_empty()).map(PathBuf::from))
}

pub fn load(cli: &Cli) -> Result<Loaded> {
    if cli.no_config {
        return Ok(Loaded::default());
    }
    let (path, explicit) = match requested_path(cli) {
        Some(p) => (p, true),
        None => match default_path() {
            Some(p) => (p, false),
            None => return Ok(Loaded::default()),
        },
    };

    match fs::read_to_string(&path) {
        Ok(text) => Ok(Loaded {
            config: parse(&text).with_context(|| format!("invalid config file {}", path.display()))?,
            path: Some(path),
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && !explicit => Ok(Loaded::default()),
        Err(err) => Err(err).with_context(|| format!("failed to read config file {}", path.display())),
    }
}

pub fn parse(text: &str) -> Result<Config> {
    Ok(toml::from_str(text.strip_prefix('\u{feff}').unwrap_or(text))?)
}

/// Writes the template to the default location (or `--config` / `$MHTML_CONFIG`)
/// and returns its path. An existing file is never overwritten.
pub fn init(cli: &Cli) -> Result<PathBuf> {
    let path = requested_path(cli)
        .or_else(default_path)
        .context("could not determine a config folder for this system")?;
    if path.exists() {
        bail!("{} already exists; edit it directly", path.display());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    }
    fs::write(&path, TEMPLATE).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

/// Merges config file and command-line flags (flags win).
pub fn resolve(cli: &Cli, loaded: Loaded) -> Result<Settings> {
    let Loaded { config: cfg, path } = loaded;
    let config_dir = path.as_deref().and_then(Path::parent);

    // Stylesheets: config files, config inline CSS, then --css, so that the most
    // specific source is last and therefore wins in the cascade.
    let mut extra_css = Vec::new();
    for spec in &cfg.css {
        extra_css.push(CssSource::load(&resolve_css_spec(spec, config_dir))?);
    }
    if let Some(css) = &cfg.custom_css {
        extra_css.push(CssSource::Inline {
            name: "config: custom_css".into(),
            css: css.clone(),
        });
    }
    for spec in &cli.css {
        extra_css.push(CssSource::load(spec)?);
    }

    let pick = |flag: &Option<String>, conf: &Option<String>| flag.clone().or_else(|| conf.clone());
    let overrides = style::overrides_css(
        pick(&cli.accent, &cfg.accent).as_deref(),
        pick(&cli.width, &cfg.width).as_deref(),
        pick(&cli.font, &cfg.font).as_deref(),
        pick(&cli.font_size, &cfg.font_size).as_deref(),
    )?;

    let toc = toggle(cli.toc, cli.no_toc).or(cfg.toc).unwrap_or(false);
    let toc_title = pick(&cli.toc_title, &cfg.toc_title).unwrap_or_else(|| "Contents".into());

    let preview_open = if cli.open {
        true
    } else if cli.no_open {
        false
    } else {
        cfg.open_browser.unwrap_or(true)
    };

    Ok(Settings {
        theme: cli.theme.or(cfg.theme).unwrap_or(Theme::Auto),
        lang: pick(&cli.lang, &cfg.lang).unwrap_or_else(|| "en".into()),
        default_style: toggle(cli.default_style, cli.no_default_style)
            .or(cfg.default_style)
            .unwrap_or(true),
        overrides,
        extra_css,
        toc_title: toc.then_some(toc_title),
        highlight: toggle(cli.highlight, cli.no_highlight).or(cfg.highlight).unwrap_or(true),
        math: toggle(cli.math, cli.no_math).or(cfg.math).unwrap_or(true),
        smart_punctuation: toggle(cli.smart_punctuation, cli.no_smart_punctuation)
            .or(cfg.smart_punctuation)
            .unwrap_or(false),
        unsafe_html: toggle(cli.unsafe_html, cli.no_unsafe_html)
            .or(cfg.unsafe_html)
            .unwrap_or(false),
        preview_open,
    })
}

/// URLs stay as they are, `~/` expands to the home folder, and relative paths
/// are relative to the config file (so a config can ship with its stylesheets).
fn resolve_css_spec(spec: &str, config_dir: Option<&Path>) -> String {
    if spec.starts_with("http://") || spec.starts_with("https://") {
        return spec.to_string();
    }
    if let Some(rest) = spec.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    let path = Path::new(spec);
    match config_dir {
        Some(dir) if path.is_relative() => dir.join(path).to_string_lossy().into_owned(),
        _ => spec.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn cli(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("mhtml").chain(args.iter().copied())).unwrap()
    }

    fn loaded(toml: &str) -> Loaded {
        Loaded {
            config: parse(toml).unwrap(),
            path: None,
        }
    }

    #[test]
    fn template_is_valid_and_changes_nothing() {
        let cfg = parse(TEMPLATE).unwrap();
        assert!(cfg.theme.is_none() && cfg.css.is_empty() && cfg.toc.is_none());
        // Uncommenting every option must also stay valid.
        let uncommented: String = TEMPLATE
            .lines()
            .map(|l| l.strip_prefix("# ").filter(|r| r.contains(" = ") || r.starts_with(':') || r.starts_with("h1") || *r == "\"\"\"").unwrap_or(l))
            .collect::<Vec<_>>()
            .join("\n");
        parse(&uncommented).unwrap_or_else(|e| panic!("{e}\n{uncommented}"));
    }

    #[test]
    fn defaults_without_config_or_flags() {
        let s = resolve(&cli(&["a.md"]), Loaded::default()).unwrap();
        assert_eq!(s.theme, Theme::Auto);
        assert_eq!(s.lang, "en");
        assert!(s.default_style && s.highlight && s.math && s.preview_open);
        assert!(!s.smart_punctuation && !s.unsafe_html && s.toc_title.is_none());
    }

    #[test]
    fn config_overrides_defaults() {
        let l = loaded("theme = \"dark\"\nlang = \"zh-TW\"\ntoc = true\ntoc_title = \"目錄\"\nmath = false\naccent = \"#e11d48\"\nopen_browser = false\n");
        let s = resolve(&cli(&["a.md"]), l).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.lang, "zh-TW");
        assert_eq!(s.toc_title.as_deref(), Some("目錄"));
        assert!(!s.math && !s.preview_open);
        assert!(s.overrides.unwrap().contains("--mhtml-accent: #e11d48;"));
    }

    #[test]
    fn flags_override_config_in_both_directions() {
        let l = loaded("theme = \"dark\"\ntoc = true\nhighlight = false\naccent = \"red\"\nopen_browser = false\n");
        let s = resolve(
            &cli(&["a.md", "--theme", "light", "--no-toc", "--highlight", "--accent", "blue", "--open"]),
            l,
        )
        .unwrap();
        assert_eq!(s.theme, Theme::Light);
        assert!(s.toc_title.is_none());
        assert!(s.highlight);
        assert!(s.overrides.unwrap().contains("blue"));
        assert!(s.preview_open);
    }

    #[test]
    fn last_of_a_flag_pair_wins() {
        let s = resolve(&cli(&["a.md", "--no-math", "--math"]), Loaded::default()).unwrap();
        assert!(s.math);
        let s = resolve(&cli(&["a.md", "--math", "--no-math"]), Loaded::default()).unwrap();
        assert!(!s.math);
    }

    #[test]
    fn unknown_keys_and_bad_types_are_rejected() {
        assert!(parse("thme = \"dark\"").is_err());
        assert!(parse("theme = \"purple\"").is_err());
        assert!(parse("toc = \"yes\"").is_err());
    }

    #[test]
    fn invalid_css_value_in_config_is_reported() {
        let l = loaded("width = \"1px; } body { display: none\"");
        assert!(resolve(&cli(&["a.md"]), l).is_err());
    }

    #[test]
    fn css_paths_resolve_relative_to_the_config_file() {
        let dir = Path::new("/cfg");
        assert_eq!(resolve_css_spec("a.css", Some(dir)), Path::new("/cfg/a.css").to_string_lossy());
        assert_eq!(resolve_css_spec("/abs/a.css", Some(dir)), "/abs/a.css");
        assert_eq!(resolve_css_spec("https://x/y.css", Some(dir)), "https://x/y.css");
    }

    #[test]
    fn custom_css_comes_before_command_line_css() {
        let dir = std::env::temp_dir().join(format!("mhtml-cfg-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("c.css"), "a{}").unwrap();
        fs::write(dir.join("f.css"), "b{}").unwrap();
        let l = Loaded {
            config: parse("css = [\"c.css\"]\ncustom_css = \"i{}\"").unwrap(),
            path: Some(dir.join("config.toml")),
        };
        let s = resolve(&cli(&["a.md", "--css", dir.join("f.css").to_str().unwrap()]), l).unwrap();
        let css: Vec<_> = s
            .extra_css
            .iter()
            .map(|c| match c {
                CssSource::Inline { css, .. } => css.as_str(),
                CssSource::Link(_) => "link",
            })
            .collect();
        assert_eq!(css, ["a{}", "i{}", "b{}"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn default_path_is_inside_an_mhtml_folder() {
        if let Some(p) = default_path() {
            assert!(p.ends_with("mhtml/config.toml") || p.ends_with("mhtml\\config.toml"));
        }
    }
}
