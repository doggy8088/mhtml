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
        let res = bin().args(args).output().unwrap();
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
