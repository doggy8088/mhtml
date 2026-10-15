mod cli;
mod config;
mod highlight;
mod page;
mod preview;
mod render;
mod sanitize;
mod style;

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_config_path {
        match config::default_path() {
            Some(path) => println!("{}", path.display()),
            None => bail!("could not determine a config folder for this system"),
        }
        return Ok(());
    }
    if cli.init_config {
        let path = config::init(&cli)?;
        println!("Created {}", path.display());
        return Ok(());
    }

    let loaded = config::load(&cli)?;
    let settings = config::resolve(&cli, loaded)?;
    let input = cli.input.as_deref().context("missing INPUT file")?;

    let source = read_input(input)?;
    let rendered = render::render(
        &source,
        &render::RenderOptions {
            smart_punctuation: settings.smart_punctuation,
            unsafe_html: settings.unsafe_html,
            highlight: settings.highlight,
            math: settings.math,
            toc_title: settings.toc_title.clone(),
        },
    );

    let title = cli
        .title
        .clone()
        .or(rendered.title.clone())
        .unwrap_or_else(|| default_title(input));

    // In preview mode the page lives in the temp folder, so point relative
    // links and images back at the Markdown file's folder.
    let preview = cli.output.is_none();
    let base = cli.base.clone().or_else(|| {
        if !preview || input == "-" {
            return None;
        }
        let dir = Path::new(input)
            .parent()
            .filter(|p| !p.as_os_str().is_empty());
        Some(preview::dir_to_file_url(dir.unwrap_or(Path::new("."))))
    });

    let html = page::Page {
        title: &title,
        lang: &settings.lang,
        body: &rendered.body,
        theme: settings.theme,
        base: base.as_deref(),
        default_style: settings.default_style,
        highlight: rendered.uses_highlight,
        overrides: settings.overrides,
        extra_css: &settings.extra_css,
    }
    .to_html();

    match cli.output.as_deref() {
        Some(out) if out == Path::new("-") => io::stdout().write_all(html.as_bytes())?,
        Some(out) => {
            fs::write(out, html).with_context(|| format!("failed to write {}", out.display()))?;
            eprintln!("Wrote {}", out.display());
            if cli.open {
                preview::open_in_browser(out)?;
            }
        }
        None => {
            let path = preview::write_temp(&html, &default_title(input))?;
            if settings.preview_open {
                eprintln!("Previewing {}", path.display());
                preview::open_in_browser(&path)?;
            } else {
                println!("{}", path.display());
            }
        }
    }
    Ok(())
}

/// Reads the Markdown source from a file or stdin (`-`), tolerating a UTF-8 BOM
/// and invalid byte sequences.
fn read_input(input: &str) -> Result<String> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read stdin")?;
    } else {
        bytes = fs::read(input).with_context(|| format!("failed to read {input}"))?;
    }
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

fn default_title(input: &str) -> String {
    if input == "-" {
        return "Markdown".into();
    }
    Path::new(input)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Markdown".into())
}
