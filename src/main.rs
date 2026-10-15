mod cli;
mod page;
mod render;
mod sanitize;
mod style;

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::Parser;

use crate::cli::Cli;
use crate::style::CssSource;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let source = read_input(&cli.input)?;
    let rendered = render::render(
        &source,
        &render::RenderOptions {
            smart_punctuation: cli.smart_punctuation,
            unsafe_html: cli.unsafe_html,
        },
    );

    let title = cli
        .title
        .clone()
        .or(rendered.title.clone())
        .unwrap_or_else(|| default_title(&cli.input));
    let extra_css = cli
        .css
        .iter()
        .map(|spec| CssSource::load(spec))
        .collect::<Result<Vec<_>>>()?;
    let overrides = style::overrides_css(
        cli.accent.as_deref(),
        cli.width.as_deref(),
        cli.font.as_deref(),
        cli.font_size.as_deref(),
    )?;
    let html = page::Page {
        title: &title,
        lang: &cli.lang,
        body: &rendered.body,
        theme: cli.theme,
        base: cli.base.as_deref(),
        default_style: !cli.no_default_style,
        overrides,
        extra_css: &extra_css,
    }
    .to_html();

    match cli.output.as_deref() {
        Some(out) if out == Path::new("-") => io::stdout().write_all(html.as_bytes())?,
        Some(out) => {
            fs::write(out, html).with_context(|| format!("failed to write {}", out.display()))?;
            eprintln!("Wrote {}", out.display());
        }
        None => bail!("preview mode is not implemented yet; pass an OUTPUT file"),
    }
    Ok(())
}

/// Reads the Markdown source from a file or stdin (`-`), tolerating a UTF-8 BOM
/// and invalid byte sequences.
fn read_input(input: &str) -> Result<String> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin().read_to_end(&mut bytes).context("failed to read stdin")?;
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
