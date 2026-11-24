//! The `mhtml` command line interface.
//!
//! Everything here is argument parsing and I/O; the conversion itself lives in
//! the library (`mhtml::to_html`).

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

use ammonia::Url;
use anyhow::{Context, Result, bail};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};

use mhtml::cli::Cli;
use mhtml::{Options, config, preview, to_html};

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

    // No INPUT: take the Markdown from a pipe. A terminal has nothing to read.
    if cli.input.is_none() && io::stdin().is_terminal() {
        missing_input();
    }

    // Both a file INPUT and piped Markdown: ambiguous, and the pipe would be
    // silently ignored (most often a forgotten `-`, as in `cat a.md | mhtml a.html`).
    if let Some(file) = cli.input.as_deref().filter(|i| *i != "-")
        && preview::stdin_has_piped_data()
    {
        Cli::command()
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "Markdown was piped into stdin but INPUT `{file}` was also given; \
                     use `-` as INPUT to read stdin (`mhtml - OUTPUT`), or drop the pipe"
                ),
            )
            .exit();
    }

    let loaded = config::load(&cli)?;
    let settings = config::resolve(&cli, loaded)?;
    let input = cli.input.as_deref().unwrap_or("-");

    if let Some(out) = cli.output.as_deref() {
        refuse_to_overwrite_input(input, out)?;
    }

    let source = read_input(input)?;
    if cli.input.is_none() && source.trim().is_empty() {
        missing_input();
    }

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
    let base = if settings.content_only {
        base.map(|base| content_base(base, cli.output.as_deref()))
            .transpose()?
    } else {
        base
    };

    let doc = to_html(
        &source,
        &Options {
            title: cli.title.clone(),
            // The file name is the last resort, after front matter and headings.
            fallback_title: Some(default_title(input)),
            lang: settings.lang.clone(),
            theme: settings.theme,
            base,
            default_style: settings.default_style,
            enhance: settings.enhance,
            content_only: settings.content_only,
            // The config file and the flags were already merged and validated.
            css_overrides: settings.overrides,
            extra_css: settings.extra_css,
            toc: settings.toc_title.is_some(),
            toc_title: settings.toc_title.clone().unwrap_or_default(),
            highlight: settings.highlight,
            math: settings.math,
            mermaid: settings.mermaid,
            smart_punctuation: settings.smart_punctuation,
            unsafe_html: settings.unsafe_html,
            ..Options::default()
        },
    )?;

    // CommonMark, and therefore GitHub, shows these asterisks literally even
    // when the CJK-friendly rules let mhtml pair them up.
    for warning in &doc.warnings {
        eprintln!(
            "warning: line {}: unpaired `{}` emphasis marker; GitHub renders it literally",
            warning.line, warning.marker
        );
    }

    match cli.output.as_deref() {
        Some(out) if out == Path::new("-") => io::stdout().write_all(doc.html.as_bytes())?,
        Some(out) => {
            fs::write(out, &doc.html)
                .with_context(|| format!("failed to write {}", out.display()))?;
            eprintln!("Wrote {}", out.display());
            if cli.open {
                preview::open_in_browser(out)?;
            }
        }
        None => {
            let path = preview::write_temp(&doc.html, &default_title(input))?;
            if settings.preview_open {
                eprintln!("Previewing {}", path.display());
                preview::open_in_browser(&path)?;
            } else {
                println!("{}", path.display());
            }
        }
    }
    if cli.check && doc.has_warnings() {
        // The HTML was produced as usual; only the exit code reports the lint.
        io::stdout().flush().ok();
        std::process::exit(1);
    }
    Ok(())
}

/// Resolves a relative --base as the browser would for the generated page.
fn content_base(base: String, output: Option<&Path>) -> Result<String> {
    if Url::parse(&base).is_ok() {
        return Ok(base);
    }
    let dir = match output {
        None => preview::directory(),
        Some(path) => path.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        &dir
    };
    let dir = std::path::absolute(dir)?;
    let document_base = Url::parse(&preview::dir_to_file_url(&dir))?;
    Ok(document_base
        .join(&base)
        .with_context(|| format!("invalid base URL {base:?}"))?
        .into())
}

/// Usage error (exit 2) for a run that has neither an INPUT nor piped Markdown.
fn missing_input() -> ! {
    Cli::command()
        .error(
            ErrorKind::MissingRequiredArgument,
            "missing INPUT: pass a Markdown file, `-`, or pipe Markdown into mhtml",
        )
        .exit()
}

/// Writing the HTML over the Markdown file it was made from would destroy the
/// source, and a mistyped OUTPUT is an easy mistake (especially in scripts).
fn refuse_to_overwrite_input(input: &str, output: &Path) -> Result<()> {
    if input == "-" || output == Path::new("-") {
        return Ok(());
    }
    // canonicalize needs the file to exist; a missing OUTPUT cannot be the input.
    if let (Ok(i), Ok(o)) = (fs::canonicalize(input), fs::canonicalize(output))
        && i == o
    {
        bail!(
            "OUTPUT ({}) is the same file as INPUT; refusing to overwrite the Markdown source",
            output.display()
        );
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
