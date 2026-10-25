# mhtml

**English** | [繁體中文](README.md)

Turn Markdown into a beautiful, **single-file, offline-friendly** HTML page. Written in Rust: one native binary for Windows, macOS and Linux, plus an npm wrapper.

```sh
mhtml README.md                    # write a temp HTML file and open it in your browser
mhtml README.md README.html        # write an HTML file
mhtml README.md out.html --theme dark --css my.css
```

<p align="center">
  <img src="https://raw.githubusercontent.com/doggy8088/mhtml/main/docs/screenshot-light.png" alt="mhtml light theme" width="49%">
  <img src="https://raw.githubusercontent.com/doggy8088/mhtml/main/docs/screenshot-dark.png" alt="mhtml dark theme" width="49%">
</p>

## Features

- **Instant preview** — give it just an input file and it opens the rendered page in your default browser (relative images keep working).
- **Light / dark themes** — `--theme auto|light|dark`; `auto` follows the system, no JavaScript involved.
- **Easy styling** — CSS variables, shortcut flags, your own `.css` files, or a config file.
- **Config file** — keep your preferences in the OS-standard config folder; command-line flags override it per run.
- **Broad syntax support** — CommonMark, GFM (tables, task lists, strikethrough, footnotes, alerts), definition lists, front matter, syntax-highlighted code, LaTeX math (MathML, offline), Mermaid diagrams (embedded Ondine renderer, works offline), table of contents.
- **Safe by default** — output is sanitized (no `<script>`, `onerror`, `javascript:`…); opt out with `--unsafe-html`.
- **Self-contained output** — CSS is inlined, no external dependencies.

## Install

```sh
npm install -g mhtml-cli     # provides the `mhtml` command
npx mhtml-cli README.md      # or run without installing
```

The unscoped `mhtml` name on npm is taken by another package, so this one is `mhtml-cli`; the command is still `mhtml`. The package is a tiny zero-dependency wrapper that downloads the native binary for your platform from GitHub Releases (SHA-256 verified). Environment variables: `MHTML_DOWNLOAD_BASE` (mirror), `MHTML_BINARY_PATH` (use a local binary), `MHTML_SKIP_DOWNLOAD`.

Or grab a binary from [Releases](https://github.com/doggy8088/mhtml/releases), or build from source (Rust 1.88+):

```sh
cargo install --git https://github.com/doggy8088/mhtml
```

## Usage

> 🤖 **For AI agents and scripts**: `mhtml --help` is a complete guide — quick start, the output contract (exit codes, stdout/stderr split), grouped examples, styling hooks, config file and troubleshooting. In automation always pass an OUTPUT (or `-`) or `--no-open`, and add `--no-config` for reproducible output.

```text
mhtml [OPTIONS] <INPUT> [OUTPUT]
```

| Usage | Meaning |
| --- | --- |
| `mhtml a.md` | Preview mode: temp file, opened in the browser |
| `mhtml a.md a.html` | Write a file (add `--open` to also open it) |
| `mhtml - -` | stdin → stdout, e.g. `cat a.md \| mhtml - -` |
| `mhtml a.md --no-open` | Preview mode without opening; prints the temp file path |

Main flags: `-t/--theme`, `-c/--css <FILE|URL>` (repeatable), `--no-default-style`, `--accent`, `--width`, `--font`, `--font-size`, `--title`, `--lang`, `--toc`, `--toc-title`, `--no-highlight`, `--no-math`, `--no-mermaid`, `--smart-punctuation`, `--unsafe-html`, `--base`, `--config`, `--no-config`, `--init-config`, `--print-config-path`. Run `mhtml --help` for details. Every on/off flag has an opposite (`--toc` / `--no-toc`) so a single run can override the config file.

## Config file

Settings can live in a config file **and** be passed as flags. Precedence:

> built-in defaults < config file < command-line flags

| OS | Default location |
| --- | --- |
| macOS | `~/Library/Application Support/mhtml/config.toml` |
| Linux | `$XDG_CONFIG_HOME/mhtml/config.toml` (usually `~/.config/mhtml/config.toml`) |
| Windows | `%APPDATA%\mhtml\config.toml` |

Override with `--config <FILE>` or `$MHTML_CONFIG`. `mhtml --init-config` creates a commented template (it never overwrites an existing file).

```toml
theme = "dark"                 # "auto" | "light" | "dark"
lang = "en"
accent = "#e11d48"
width = "900px"
font = "Georgia, serif"
font_size = "17px"
toc = true
toc_title = "Contents"
highlight = true
math = true
mermaid = true
smart_punctuation = false
unsafe_html = false
default_style = true
open_browser = true
css = ["my-style.css"]        # relative to this file; "~/" and http(s) URLs work
custom_css = """
h1 { letter-spacing: .02em; }
"""
```

Unknown keys are rejected with a list of valid ones. Stylesheets apply in this order: config `css` → `custom_css` → command-line `--css`.

## Custom styling

The built-in style is driven by CSS custom properties:

```css
:root {
  --mhtml-accent: #e11d48;
  --mhtml-max-width: 70ch;
  --mhtml-font: Georgia, serif;
}
@media (prefers-color-scheme: dark) { :root { --mhtml-bg: #000; } }
```

Available: `--mhtml-bg`, `-fg`, `-muted`, `-border`, `-accent`, `-code-bg`, `-code-fg`, `-inline-code-bg`, `-quote-border`, `-table-stripe`, `-mark`, `-note`, `-tip`, `-important`, `-warning`, `-caution`, `-font`, `-mono`, `-font-size`, `-line-height`, `-max-width`, `-radius`. `<html>` carries `data-theme="auto|light|dark"`; syntax tokens use `.hl-*` classes.

## Development

```sh
make help      # list all common tasks
make check     # fmt + clippy + all tests (same as CI)
make preview   # preview examples/example.md in the browser
make release   # optimized build at target/release/mhtml
make tag       # verify versions, then create the local vX.Y.Z tag
```

Release: bump the version in `Cargo.toml` and `npm/package.json`, then `git tag vX.Y.Z && git push --tags`. GitHub Actions builds six binaries, creates the Release and publishes the npm package via npm Trusted Publishing (OIDC, no token secret needed).

## License

[MIT](LICENSE) © 2026 Will 保哥
