//! Preview mode: write the page to a temp file and open it in the browser.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

/// Temp files older than this are removed on the next preview.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Writes `html` into `<tmp>/mhtml/` and returns the file path.
///
/// The file must outlive this process (the browser reads it later), so it is
/// not deleted on exit; stale files from earlier runs are swept instead.
pub fn write_temp(html: &str, stem: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("mhtml");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    sweep(&dir);

    let path = dir.join(format!("{}-{}.html", sanitize_stem(stem), std::process::id()));
    fs::write(&path, html).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn open_in_browser(path: &Path) -> Result<()> {
    open::that(path).with_context(|| format!("failed to open {} in a browser", path.display()))
}

fn sweep(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > MAX_AGE);
        if old && entry.path().extension().is_some_and(|e| e == "html") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn sanitize_stem(stem: &str) -> String {
    let s: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if s.is_empty() { "preview".into() } else { s }
}

/// `file:///…/dir/` URL for a directory, usable as `<base href>` so that
/// relative images and links keep working from the temp file.
pub fn dir_to_file_url(dir: &Path) -> String {
    let abs = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let mut s = abs.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string(); // Windows verbatim prefix
    }
    let mut url = String::from("file://");
    if !s.starts_with('/') {
        url.push('/'); // Windows drive letter: file:///C:/…
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => url.push(b as char),
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_is_percent_encoded_and_slash_terminated() {
        let url = dir_to_file_url(Path::new("/definitely/not/here/我的 docs"));
        assert!(url.starts_with("file:///definitely/not/here/"), "{url}");
        assert!(url.ends_with("%E6%88%91%E7%9A%84%20docs/"), "{url}");
    }

    #[test]
    fn stem_is_made_filename_safe() {
        assert_eq!(sanitize_stem("my doc/v1"), "my-doc-v1");
        assert_eq!(sanitize_stem(""), "preview");
    }

    #[test]
    fn temp_file_is_written() {
        let path = write_temp("<p>hi</p>", "unit test").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "<p>hi</p>");
        fs::remove_file(path).unwrap();
    }
}
