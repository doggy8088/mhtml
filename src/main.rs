mod cli;
mod page;
mod preview;
mod render;
mod sanitize;
mod style;

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
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
    // In preview mode the page lives in the temp folder, so point relative
    // links and images back at the Markdown file's folder.
    let preview = cli.output.is_none();
    let base = cli.base.clone().or_else(|| {
        if !preview || cli.input == "-" {
            return None;
        }
        let dir = Path::new(&cli.input).parent().filter(|p| !p.as_os_str().is_empty());
        Some(preview::dir_to_file_url(dir.unwrap_or(Path::new("."))))
    });
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
        base: base.as_deref(),
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
            if cli.open {
                preview::open_in_browser(out)?;
            }
        }
        None => {
            let path = preview::write_temp(&html, &default_title(&cli.input))?;
            if cli.no_open {
                println!("{}", path.display());
            } else {
                eprintln!("Previewing {}", path.display());
                preview::open_in_browser(&path)?;
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
