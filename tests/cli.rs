//! End-to-end tests that run the compiled `mhtml` binary.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mhtml"));
    // Never let a developer's real config file leak into the tests.
    cmd.env_remove("MHTML_CONFIG");
    cmd
}

/// A scratch directory that is removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mhtml-it-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
    fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, content).unwrap();
        path
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn converts_markdown_file_to_html_file() {
    let dir = Scratch::new("convert");
    let md = dir.file("a.md", "# Hello\n\nworld **bold**\n");
    let out = dir.path().join("a.html");

    let res = bin()
        .arg("--no-config")
        .arg(&md)
        .arg(&out)
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", stderr(&res));

    let html = fs::read_to_string(&out).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>Hello</title>"));
    assert!(html.contains("<h1 id=\"hello\">Hello</h1>"));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<style id=\"mhtml-base\">"));
    assert!(!html.contains("<base "), "no base tag outside preview mode");
    assert!(!html.contains("<script"), "no script for a plain page");
}

#[test]
fn reads_stdin_and_writes_stdout() {
    let mut child = bin()
        .args(["--no-config", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("# 你好\n".as_bytes())
        .unwrap();
    let res = child.wait_with_output().unwrap();
    assert!(res.status.success());
    let html = stdout(&res);
    assert!(html.contains("<title>你好</title>"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
}

#[test]
fn themes_switch_palettes() {
    let dir = Scratch::new("theme");
    let md = dir.file("a.md", "x\n");
    let run = |theme: &str| {
        let res = bin()
            .args(["--no-config", "-t", theme])
            .arg(&md)
            .arg("-")
            .output()
            .unwrap();
        assert!(res.status.success(), "{}", stderr(&res));
        stdout(&res)
    };

    let dark = run("dark");
    assert!(dark.contains("data-theme=\"dark\"") && !dark.contains("prefers-color-scheme: dark"));
    let light = run("light");
    assert!(
        light.contains("data-theme=\"light\"") && !light.contains("prefers-color-scheme: dark")
    );
    let auto = run("auto");
    assert!(auto.contains("data-theme=\"auto\"") && auto.contains("prefers-color-scheme: dark"));
}

#[test]
fn custom_css_files_and_urls_come_after_the_default_style() {
    let dir = Scratch::new("css");
    let md = dir.file("a.md", "x\n");
    let css = dir.file("mine.css", "h1 { color: hotpink }");

    let res = bin()
        .args(["--no-config", "--accent", "#e11d48", "--width", "70ch"])
        .arg("--css")
        .arg(&css)
        .args(["--css", "https://example.com/x.css"])
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let html = stdout(&res);

    let base = html.find("id=\"mhtml-base\"").unwrap();
    let inline = html.find("h1 { color: hotpink }").unwrap();
    let link = html
        .find("<link rel=\"stylesheet\" href=\"https://example.com/x.css\">")
        .unwrap();
    assert!(base < inline && inline < link);
    assert!(html.contains("--mhtml-accent: #e11d48;"));
    assert!(html.contains("--mhtml-max-width: 70ch;"));
}

#[test]
fn no_default_style_leaves_only_user_css() {
    let dir = Scratch::new("nostyle");
    let md = dir.file("a.md", "x\n");
    let css = dir.file("mine.css", "body{margin:0}");
    let res = bin()
        .args(["--no-config", "--no-default-style", "--css"])
        .arg(&css)
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    let html = stdout(&res);
    assert!(!html.contains("mhtml-base") && !html.contains("mhtml-theme"));
    assert!(html.contains("body{margin:0}"));
}

#[test]
fn raw_html_is_sanitized_unless_unsafe() {
    let dir = Scratch::new("xss");
    let md = dir.file(
        "a.md",
        "<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n",
    );
    let run = |extra: &[&str]| {
        let res = bin()
            .arg("--no-config")
            .args(extra)
            .arg(&md)
            .arg("-")
            .output()
            .unwrap();
        stdout(&res)
    };
    let safe = run(&[]);
    assert!(!safe.contains("alert(1)") && !safe.contains("onerror"));
    assert!(run(&["--unsafe-html"]).contains("<script>alert(1)</script>"));
}

#[test]
fn toc_math_and_highlight_work_together() {
    let dir = Scratch::new("features");
    let md = dir.file(
        "a.md",
        "# T\n\n## Part\n\n$x^2$\n\n```rust\nfn main() {}\n```\n",
    );
    let res = bin()
        .args(["--no-config", "--toc", "--toc-title", "目錄"])
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    let html = stdout(&res);
    assert!(html.contains("<strong>目錄</strong>"));
    assert!(html.contains("<a href=\"#part\">Part</a>"));
    assert!(html.contains("<msup>"));
    assert!(html.contains("<span class=\"hl-"));
    assert!(html.contains(".hl-"), "highlight css is embedded");

    let res = bin()
        .args(["--no-config", "--no-math", "--no-highlight"])
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    let html = stdout(&res);
    assert!(!html.contains("<msup>") && !html.contains("<span class=\"hl-"));
}

#[test]
fn loose_inline_math_and_cjk_adjacency_render() {
    let dir = Scratch::new("loosemath");
    let md = dir.file(
        "a.md",
        "中文$L$中文 and $ L $ and $ a + b $\n\n價格 $100 元和 $200 元\n",
    );
    let res = bin().arg("--no-config").arg(&md).arg("-").output().unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let html = stdout(&res);
    assert_eq!(html.matches("<math").count(), 3, "{html}");
    assert!(!html.contains("$ L $"), "{html}");
    assert!(html.contains("$100 元和 $200 元"), "{html}");
    assert!(html.contains("中文<math"), "{html}");
    assert!(html.contains("</math>中文"), "{html}");
}

#[test]
fn mermaid_diagrams_get_the_embedded_renderer() {
    let dir = Scratch::new("mermaid");
    let md = dir.file(
        "a.md",
        "# Diagrams\n\n```mermaid\nflowchart LR\n  A --> B\n```\n",
    );
    let run = |extra: &[&str]| {
        let res = bin()
            .arg("--no-config")
            .args(extra)
            .arg(&md)
            .arg("-")
            .output()
            .unwrap();
        assert!(res.status.success(), "{}", stderr(&res));
        stdout(&res)
    };

    let html = run(&[]);
    assert!(html.contains("<div class=\"mermaid-diagram\">"), "{html}");
    assert!(html.contains("language-mermaid"), "{html}");
    assert!(html.contains("A --&gt; B"), "{html}");
    assert!(html.contains("id=\"mhtml-mermaid-wasm\""), "{html}");
    assert!(html.contains(">AGFzbQ"), "the WASM is embedded: {html}");
    assert!(html.contains("Ondine.create"), "{html}");
    assert!(
        html.contains("querySelectorAll('.mermaid-diagram')"),
        "{html}"
    );

    let plain = run(&["--no-mermaid"]);
    assert!(
        !plain.contains("<div class=\"mermaid-diagram\">"),
        "{plain}"
    );
    assert!(!plain.contains("mhtml-mermaid-wasm"), "{plain}");
    assert!(plain.contains("language-mermaid"), "{plain}");
    assert!(plain.contains("A --&gt; B"), "{plain}");
}

#[test]
fn preview_mode_writes_temp_file_with_base() {
    let dir = Scratch::new("preview");
    let md = dir.file("a.md", "# Preview\n\n[jump](#preview)\n");
    let res = bin()
        .args(["--no-config", "--no-open"])
        .arg(&md)
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", stderr(&res));

    let tmp = PathBuf::from(stdout(&res).trim());
    assert!(tmp.exists(), "{}", tmp.display());
    let html = fs::read_to_string(&tmp).unwrap();
    assert!(html.contains("<base href=\"file://"), "{html}");
    assert!(html.contains("scrollIntoView"));
    let _ = fs::remove_file(tmp);
}

/// Runs mhtml with `args`, feeding `input` to stdin.
fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let mut child = bin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn piped_markdown_without_input_previews_to_a_content_named_temp_file() {
    let version = env!("CARGO_PKG_VERSION");
    let args = ["--no-config", "--no-open"];

    // INPUT omitted: stdin is the Markdown, and `-` behaves the same.
    let res = run_with_stdin(&args, "# Piped 你好\n\nbody\n");
    assert!(res.status.success(), "{}", stderr(&res));
    let path = PathBuf::from(stdout(&res).trim());
    assert_eq!(stdout(&res).trim().lines().count(), 1);
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with(&format!("Markdown-{version}-")) && name.ends_with(".html"),
        "{name}"
    );
    let html = fs::read_to_string(&path).unwrap();
    assert!(html.contains("<title>Piped 你好</title>"), "{html}");

    let with_dash = run_with_stdin(&["--no-config", "--no-open", "-"], "# Piped 你好\n\nbody\n");
    assert_eq!(stdout(&with_dash), stdout(&res), "same content, same file");

    // Other content or other flags give another file; nothing is overwritten.
    let other = run_with_stdin(&args, "# Another\n");
    let dark = run_with_stdin(
        &["--no-config", "--no-open", "--theme", "dark"],
        "# Piped 你好\n\nbody\n",
    );
    assert_ne!(stdout(&other), stdout(&res));
    assert_ne!(stdout(&dark), stdout(&res));
    for out in [&res, &other, &dark] {
        let _ = fs::remove_file(stdout(out).trim());
    }

    // Empty stdin is a usage error, not an empty page.
    let empty = run_with_stdin(&args, "  \n");
    assert_eq!(empty.status.code(), Some(2));
    assert!(empty.stdout.is_empty());
    assert!(
        stderr(&empty).contains("missing INPUT"),
        "{}",
        stderr(&empty)
    );
}

#[cfg(unix)]
#[test]
fn piped_stdin_together_with_an_input_file_is_a_usage_error() {
    let dir = Scratch::new("pipe-conflict");
    let md = dir.file("a.md", "# File\n");
    let md = md.to_str().unwrap();

    // `cat a.md | mhtml out.html`: the pipe would be ignored, so refuse.
    let res = run_with_stdin(&["--no-config", "--no-open", md], "# Piped\n");
    assert_eq!(res.status.code(), Some(2), "{}", stderr(&res));
    assert!(res.stdout.is_empty());
    assert!(
        stderr(&res).contains("use `-` as INPUT"),
        "{}",
        stderr(&res)
    );

    // Same with an OUTPUT, and the order of the flags does not matter.
    let out = dir.path().join("o.html");
    let res = run_with_stdin(&["--no-config", md, out.to_str().unwrap()], "# Piped\n");
    assert_eq!(res.status.code(), Some(2));
    assert!(!out.exists());

    // `-` is the explicit way to read the pipe, and stays valid.
    let res = run_with_stdin(&["--no-config", "-", "-"], "# Piped\n");
    assert!(res.status.success(), "{}", stderr(&res));

    // Nothing piped (null or idle stdin) is not a conflict.
    let res = bin()
        .args(["--no-config", md, "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let mut child = bin()
        .args(["--no-config", md, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let idle = child.stdin.take(); // open pipe, nothing written
    let res = child.wait_with_output().unwrap();
    drop(idle);
    assert!(res.status.success());
}

#[test]
fn config_file_is_applied_and_flags_override_it() {
    let dir = Scratch::new("config");
    let md = dir.file("a.md", "# T\n\n## A\n");
    let cfg = dir.file(
        "config.toml",
        "theme = \"dark\"\nlang = \"zh-TW\"\ntoc = true\ntoc_title = \"目錄\"\ncss = [\"extra.css\"]\n",
    );
    dir.file("extra.css", ".from-config{}");

    let run = |extra: &[&str]| {
        let res = bin()
            .arg("--config")
            .arg(&cfg)
            .args(extra)
            .arg(&md)
            .arg("-")
            .output()
            .unwrap();
        assert!(res.status.success(), "{}", stderr(&res));
        stdout(&res)
    };

    let html = run(&[]);
    assert!(html.contains("data-theme=\"dark\"") && html.contains("lang=\"zh-TW\""));
    assert!(html.contains("<strong>目錄</strong>"));
    assert!(
        html.contains(".from-config{}"),
        "css is resolved next to the config file"
    );

    let html = run(&["--theme", "light", "--no-toc", "--lang", "en"]);
    assert!(html.contains("data-theme=\"light\"") && html.contains("lang=\"en\""));
    assert!(!html.contains("class=\"toc\""));
}

#[test]
fn invalid_config_and_missing_explicit_config_fail_clearly() {
    let dir = Scratch::new("badconfig");
    let md = dir.file("a.md", "x\n");
    let bad = dir.file("bad.toml", "thme = \"dark\"\n");

    let res = bin()
        .arg("--config")
        .arg(&bad)
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(!res.status.success());
    assert!(
        stderr(&res).contains("unknown field `thme`"),
        "{}",
        stderr(&res)
    );

    let res = bin()
        .arg("--config")
        .arg(dir.path().join("missing.toml"))
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(!res.status.success());
    assert!(stderr(&res).contains("failed to read config file"));
}

// The Windows config folder comes from the Known Folder API, not $APPDATA.
#[cfg(unix)]
#[test]
fn init_config_creates_template_once_in_the_os_config_folder() {
    let dir = Scratch::new("init");
    let run = || {
        bin()
            .arg("--init-config")
            .env("HOME", dir.path())
            .env("XDG_CONFIG_HOME", dir.path().join("xdg"))
            .env("APPDATA", dir.path().join("appdata"))
            .output()
            .unwrap()
    };

    let first = run();
    assert!(first.status.success(), "{}", stderr(&first));
    let path = PathBuf::from(stdout(&first).trim().strip_prefix("Created ").unwrap());
    assert!(path.starts_with(dir.path()), "{}", path.display());
    assert!(path.ends_with(Path::new("mhtml").join("config.toml")));
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("# theme = \"auto\"")
    );

    let printed = bin()
        .arg("--print-config-path")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("xdg"))
        .env("APPDATA", dir.path().join("appdata"))
        .output()
        .unwrap();
    assert_eq!(stdout(&printed).trim(), path.to_str().unwrap());

    let second = run();
    assert!(!second.status.success());
    assert!(stderr(&second).contains("already exists"));
}

#[test]
fn errors_for_missing_input_and_bad_flag_values() {
    let res = bin()
        .arg("--no-config")
        .arg("/definitely/missing.md")
        .arg("-")
        .output()
        .unwrap();
    assert!(!res.status.success());
    assert!(stderr(&res).contains("failed to read"));

    let dir = Scratch::new("badflag");
    let md = dir.file("a.md", "x\n");
    let res = bin()
        .args(["--no-config", "--accent", "red; } body{display:none"])
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(!res.status.success());
    assert!(stderr(&res).contains("invalid value for accent"));
}

#[test]
fn example_document_renders_every_feature() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/example.md");
    let res = bin()
        .args(["--no-config", "--toc"])
        .arg(&example)
        .arg("-")
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let html = stdout(&res);
    for needle in [
        "<title>mhtml 範例文件</title>",
        "markdown-alert-note",
        "<table>",
        "type=\"checkbox\"",
        "<kbd>",
        "<mfrac>",
        "footnote-definition",
        "<details>",
        "<dl>",
        "class=\"toc\"",
        "<span class=\"hl-",
        "class=\"mermaid-diagram\"",
        "id=\"mhtml-mermaid-wasm\"",
    ] {
        assert!(html.contains(needle), "missing {needle}");
    }
}

#[test]
fn refuses_to_overwrite_the_input_file() {
    let dir = Scratch::new("same");
    let md = dir.file("a.md", "# keep me\n");

    let res = bin().arg("--no-config").arg(&md).arg(&md).output().unwrap();
    assert!(!res.status.success());
    assert!(
        stderr(&res).contains("same file as INPUT"),
        "{}",
        stderr(&res)
    );
    assert_eq!(
        fs::read_to_string(&md).unwrap(),
        "# keep me\n",
        "source must be untouched"
    );

    // The same file reached through a different path is caught too.
    let alias = dir.path().join(".").join("a.md");
    let res = bin()
        .arg("--no-config")
        .arg(&md)
        .arg(&alias)
        .output()
        .unwrap();
    assert!(!res.status.success());
    assert_eq!(fs::read_to_string(&md).unwrap(), "# keep me\n");
}

/// The OUTPUT CONTRACT section of `mhtml --help` promises these exact behaviors.
#[test]
fn output_contract_in_the_help_is_true() {
    let dir = Scratch::new("contract");
    let md = dir.file("a.md", "# Title\n");
    let out = dir.path().join("a.html");

    // success with an OUTPUT file: exit 0, stdout empty, "Wrote <path>" on stderr
    let res = bin()
        .arg("--no-config")
        .arg(&md)
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(res.status.code(), Some(0));
    assert!(res.stdout.is_empty());
    assert!(stderr(&res).starts_with("Wrote "), "{}", stderr(&res));

    // success with OUTPUT `-`: stdout is exactly one HTML document, stderr quiet
    let res = bin().arg("--no-config").arg(&md).arg("-").output().unwrap();
    assert_eq!(res.status.code(), Some(0));
    let html = stdout(&res);
    assert!(html.starts_with("<!doctype html>") && html.trim_end().ends_with("</html>"));
    assert_eq!(html.matches("<!doctype html>").count(), 1);
    assert!(res.stderr.is_empty());

    // preview with --no-open: stdout is only the temp file path
    let res = bin()
        .args(["--no-config", "--no-open"])
        .arg(&md)
        .output()
        .unwrap();
    assert_eq!(res.status.code(), Some(0));
    let printed = stdout(&res);
    assert_eq!(printed.trim().lines().count(), 1);
    let _ = fs::remove_file(printed.trim());

    // failure: exit 1, stdout empty, "Error: ..." on stderr
    let res = bin()
        .arg("--no-config")
        .arg(dir.path().join("missing.md"))
        .arg("-")
        .output()
        .unwrap();
    assert_eq!(res.status.code(), Some(1));
    assert!(res.stdout.is_empty());
    assert!(stderr(&res).starts_with("Error: "), "{}", stderr(&res));

    // output folder must exist
    let res = bin()
        .arg("--no-config")
        .arg(&md)
        .arg(dir.path().join("nodir/x.html"))
        .output()
        .unwrap();
    assert_eq!(res.status.code(), Some(1));

    // bad usage: exit 2
    for args in [
        &["--no-such-flag"][..],
        &[][..],
        &["a.md", "--open", "--no-open"][..],
    ] {
        // A null stdin is "nothing piped", whatever the test runner provides.
        let res = bin().args(args).stdin(Stdio::null()).output().unwrap();
        assert_eq!(res.status.code(), Some(2), "{args:?}");
        assert!(res.stdout.is_empty());
    }
}

#[test]
fn help_is_a_guide_for_agents_and_short_help_points_to_it() {
    let long = bin().arg("--help").output().unwrap();
    assert_eq!(long.status.code(), Some(0));
    let long = stdout(&long);
    assert!(long.starts_with(&format!(
        "mhtml {}\nUsage: mhtml [OPTIONS] <INPUT> [OUTPUT]",
        env!("CARGO_PKG_VERSION")
    )));
    // The most important guidance comes before the option list.
    let quick = long.find("QUICK START").unwrap();
    let contract = long.find("OUTPUT CONTRACT").unwrap();
    let options = long.find("Options:").unwrap();
    assert!(quick < contract && contract < options);
    assert!(long.find("TROUBLESHOOTING").unwrap() > options);

    let short = stdout(&bin().arg("-h").output().unwrap());
    assert!(
        short.contains("mhtml --help"),
        "short help must point to the full guide"
    );
    assert!(!short.contains("OUTPUT CONTRACT") && short.lines().count() < long.lines().count() / 3);
}

#[test]
fn adaptive_presentation_is_on_by_default_and_can_be_turned_off() {
    let dir = Scratch::new("enhance");
    let md = dir.file(
        "a.md",
        "# 文件\n\n- **背景**：這份文件說明專案的需求與驗收方向\n- **日期**：今天開會確認的事項\n\n| 名稱 | 值 |\n| --- | --- |\n| Alpha | 2 |\n",
    );
    let run = |args: &[&str]| {
        let res = bin()
            .arg("--no-config")
            .args(args)
            .arg(&md)
            .arg("-")
            .output()
            .unwrap();
        assert!(res.status.success(), "{}", stderr(&res));
        stdout(&res)
    };

    let html = run(&[]);
    assert!(html.contains("<style id=\"mhtml-enhance\">"));
    assert!(
        html.contains("<script id=\"mhtml-enhance-js\">"),
        "tables get controls"
    );
    assert!(html.contains("class=\"markdown-body is-cjk\""), "{html}");
    assert!(
        html.contains("lang=\"zh-Hant\""),
        "CJK text sets the language"
    );
    assert!(html.contains("<ul class=\"kv\">") && html.contains("table-wrap"));
    assert!(html.contains("<td>Alpha</td>"));

    // An explicit language always wins over detection.
    assert!(run(&["--lang", "en"]).contains("lang=\"en\""));

    for args in [&["--no-enhance"][..], &["--no-default-style"][..]] {
        let html = run(args);
        assert!(!html.contains("mhtml-enhance"), "{args:?}");
        assert!(!html.contains("id=\"mhtml-adaptive\""), "{args:?}");
        assert!(
            !html.contains("table-wrap") && !html.contains("class=\"kv\""),
            "{args:?}"
        );
        assert!(html.contains("<main class=\"markdown-body\">"), "{args:?}");
        assert!(html.contains("<td>Alpha</td>"));
    }
    assert!(
        run(&["--no-enhance"]).contains("lang=\"en\""),
        "no detection when off"
    );

    // `enhance = false` in the config, and --enhance to undo it.
    let cfg = dir.file("c.toml", "enhance = false\n");
    let res = bin()
        .arg("--config")
        .arg(&cfg)
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(!stdout(&res).contains("mhtml-enhance"));
    let res = bin()
        .arg("--config")
        .arg(&cfg)
        .arg("--enhance")
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(stdout(&res).contains("mhtml-enhance"));
}

#[test]
fn code_blocks_get_a_copy_icon_but_inline_code_does_not() {
    let dir = Scratch::new("copy-icon");
    let md = dir.file("a.md", "# T\n\nUse `inline` here.\n\n```sh\nls\n```\n");
    let res = bin().arg("--no-config").arg(&md).arg("-").output().unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let html = stdout(&res);
    // The script builds one copy button per code block and copies its text.
    assert!(html.contains("code-copy"), "{html}");
    assert!(html.contains("writeText"), "clipboard copy: {html}");
    assert!(
        html.contains("document.querySelectorAll('.markdown-body pre').forEach(setupCode)"),
        "code blocks only: {html}"
    );
    // Hidden by default; revealed while the block is hovered or the button focused.
    assert!(html.contains(".code-block:hover .code-copy"), "{html}");
    assert!(html.contains("opacity: 0;"), "hidden by default: {html}");
    // Inline code spans stay untouched.
    assert!(html.contains("<code>inline</code>"), "{html}");
    let res = bin()
        .arg("--no-config")
        .arg("--no-enhance")
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(!stdout(&res).contains("code-copy"));
}

#[test]
fn tables_sort_from_two_rows_and_support_column_drag() {
    let dir = Scratch::new("col-drag");
    let md = dir.file(
        "a.md",
        "# T\n\n| a | b |\n| --- | --- |\n| 2 | x |\n| 1 | y |\n",
    );
    let res = bin().arg("--no-config").arg(&md).arg("-").output().unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let html = stdout(&res);
    // Two body rows are sortable now (the gate used to be five).
    assert!(html.contains("table-wrap sortable"), "{html}");
    // Column drag: pointer move, keyboard move, and restore are all wired.
    assert!(
        html.contains("moveColumnBy") && html.contains("resetColumns"),
        "{html}"
    );
    assert!(html.contains("is-drop-before"), "{html}");
    // Off entirely without enhancement.
    let res = bin()
        .arg("--no-config")
        .arg("--no-enhance")
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert!(!stdout(&res).contains("is-drop-before"));
}

/// Four unpaired emphasis attempts (lines 4, 5, 12, 21) among clean content.
const UNPAIRED_EMPHASIS_DOC: &str = "\
# 狀態報告

以下是本週的重點整理。
**審查執行：**2026-11-04，全數通過
這是**「引號」**文字，行內還有 `**code：**x` 程式碼。

- **正常**：項目
- 星號可以跳脫 \\* 與 a * b * c 數學寫法

| 欄位 | 值 |
| --- | --- |
| **狀態：**待確認 | ok |
| plain | ok |

## 章節

一般段落。

> 引用沒有問題。

**注意：**表格已更新

結尾。
";

#[test]
fn unpaired_emphasis_is_warned_on_stderr_with_line_numbers() {
    let dir = Scratch::new("emph-warn");
    let md = dir.file("a.md", UNPAIRED_EMPHASIS_DOC);

    let res = bin().arg("--no-config").arg(&md).arg("-").output().unwrap();
    assert!(res.status.success(), "{}", stderr(&res));
    let err = stderr(&res);
    let warnings: Vec<&str> = err.lines().filter(|l| l.starts_with("warning: ")).collect();
    assert_eq!(warnings.len(), 4, "{err}");
    for line in [4, 5, 12, 21] {
        assert!(
            warnings
                .iter()
                .any(|w| w.contains(&format!("line {line}:"))),
            "missing line {line}: {warnings:?}"
        );
    }
    // The CJK-friendly fix renders the spots and never leaks its HTML comment.
    let html = stdout(&res);
    assert!(!html.contains("<!---->"), "{html}");
    assert!(html.contains("<strong>審查執行：</strong>"), "{html}");
    assert!(html.contains("<strong>「引號」</strong>"), "{html}");

    let res = bin()
        .arg("--no-config")
        .arg("--unsafe-html")
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    let html = stdout(&res);
    assert!(!html.contains("<!---->"), "{html}");
    assert!(html.contains("<strong>狀態：</strong>待確認"), "{html}");
}

#[test]
fn check_flag_gates_the_exit_code_on_emphasis_warnings() {
    let dir = Scratch::new("emph-check");
    let md = dir.file("a.md", UNPAIRED_EMPHASIS_DOC);
    let clean = dir.file("clean.md", "# 乾淨\n\n**正常** 與 *斜體*。\n");

    let res = bin()
        .args(["--no-config", "--check"])
        .arg(&md)
        .arg("-")
        .output()
        .unwrap();
    assert_ne!(res.status.code(), Some(0), "{}", stderr(&res));

    let res = bin()
        .args(["--no-config", "--check"])
        .arg(&clean)
        .arg("-")
        .output()
        .unwrap();
    assert_eq!(res.status.code(), Some(0), "{}", stderr(&res));
}
