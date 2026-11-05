mod cli;
mod config;
mod enhance;
mod highlight;
mod page;
mod preview;
mod render;
mod sanitize;
mod style;

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};

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
    let rendered = render::render(
        &source,
        &render::RenderOptions {
            smart_punctuation: settings.smart_punctuation,
            unsafe_html: settings.unsafe_html,
            highlight: settings.highlight,
            math: settings.math,
            mermaid: settings.mermaid,
            toc_title: settings.toc_title.clone(),
            enhance: settings.enhance && settings.default_style,
        },
    );
    // CommonMark, and therefore GitHub, shows these asterisks literally even
    // when the CJK-friendly rules let mhtml pair them up.
    for warning in &rendered.warnings {
        eprintln!(
            "warning: line {}: unpaired `{}` emphasis marker; GitHub renders it literally",
            warning.line, warning.marker
        );
    }
    let lang = settings
        .lang
        .clone()
        .or_else(|| {
            settings
                .enhance
                .then(|| enhance::detect_lang(&source))
                .flatten()
                .map(String::from)
        })
        .unwrap_or_else(|| "en".into());
    let main_class = rendered.profile.classes.join(" ");

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
        lang: &lang,
        body: &rendered.body,
        theme: settings.theme,
        base: base.as_deref(),
        default_style: settings.default_style,
        enhance: settings.enhance,
        main_class: &main_class,
        outline: rendered.outline.as_deref(),
        interactive: rendered.profile.interactive,
        title_banner: !rendered.has_h1 && rendered.profile.classes.contains(&"doc-prose"),
        highlight: rendered.uses_highlight,
        mermaid: rendered.uses_mermaid,
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
    if cli.check && !rendered.warnings.is_empty() {
        // The HTML was produced as usual; only the exit code reports the lint.
        io::stdout().flush().ok();
        std::process::exit(1);
    }
    Ok(())
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
