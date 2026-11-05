//! Preview mode: write the page to a temp file and open it in the browser.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

/// Temp files older than this are removed on the next preview.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Writes `html` into `<tmp>/mhtml/` and returns the file path.
///
/// The name is `<stem>-<mhtml version>-<hash of the html>.html`: the same page
/// from the same mhtml version always lands on the same file, while a new
/// version, source, theme or flag gives a new one, so nothing is overwritten
/// behind an open browser tab.
///
/// The file must outlive this process (the browser reads it later), so it is
/// not deleted on exit; stale files from earlier runs are swept instead.
pub fn write_temp(html: &str, stem: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("mhtml");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    sweep(&dir);

    let path = dir.join(temp_name(html, stem));
    // Always (re)write, even when the file exists: that refreshes its mtime, so
    // the sweep above never removes a page that was just requested again.
    fs::write(&path, html).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn temp_name(html: &str, stem: &str) -> String {
    format!(
        "{}-{}-{:016x}.html",
        sanitize_stem(stem),
        env!("CARGO_PKG_VERSION"),
        fnv1a64(html.as_bytes())
    )
}

/// FNV-1a, 64 bit. Not cryptographic; it only has to be stable and spread
/// names well (`DefaultHasher` makes no promise across Rust releases).
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub fn open_in_browser(path: &Path) -> Result<()> {
    open::that(path).with_context(|| format!("failed to open {} in a browser", path.display()))
}

fn sweep(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if s.is_empty() { "preview".into() } else { s }
}

/// True when stdin is a pipe or file that already holds data, i.e. the caller
/// piped Markdown into mhtml. A terminal, `/dev/null`, and an empty or idle
/// pipe (common for CI and agent child processes) all count as "nothing piped".
///
/// Only implemented on Unix; elsewhere it reports false.
#[cfg(unix)]
pub fn stdin_has_piped_data() -> bool {
    // A producer such as `cat` may start a moment after mhtml; wait briefly
    // for its first bytes. An idle pipe costs this much once, and only when
    // an INPUT file was also given.
    const WAIT_MS: libc::c_int = 150;
    let fd = 0;
    // SAFETY: `stat` is plain old data and `fstat`/`poll` only read/write the
    // structs we pass; fd 0 is valid to query even when closed (they just fail).
    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut st) != 0 {
            return false;
        }
        match st.st_mode & libc::S_IFMT {
            libc::S_IFREG => st.st_size > 0,
            libc::S_IFIFO => {
                let mut p = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                libc::poll(&mut p, 1, WAIT_MS) > 0 && p.revents & libc::POLLIN != 0
            }
            _ => false,
        }
    }
}

#[cfg(not(unix))]
pub fn stdin_has_piped_data() -> bool {
    false
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
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                url.push(b as char)
            }
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
    fn temp_name_is_stable_versioned_and_content_addressed() {
        let a = temp_name("<p>a</p>", "doc");
        assert_eq!(a, temp_name("<p>a</p>", "doc"), "same page, same name");
        assert_ne!(a, temp_name("<p>b</p>", "doc"), "other page, other name");
        let version = env!("CARGO_PKG_VERSION");
        assert!(a.starts_with(&format!("doc-{version}-")), "{a}");
        assert!(a.ends_with(".html"), "{a}");
        // FNV-1a 64 test vectors keep the hash stable across releases.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn temp_file_is_written() {
        let path = write_temp("<p>hi</p>", "unit test").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "<p>hi</p>");
        fs::remove_file(path).unwrap();
    }
}
