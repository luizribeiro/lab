//! Path canonicalization for inference events.
//!
//! Backends observe paths in arbitrary forms (relative, symlinked, with
//! `.`/`..` components). Before events are deduplicated or written to
//! a config, paths must be normalized to an absolute, symlink-resolved
//! canonical form.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

/// Canonicalize an observed path.
///
/// - If the path exists, returns `std::fs::canonicalize(path)`.
/// - If the path does not exist (e.g. a `create` or `delete` event for
///   a file that was just created or just removed), walks ancestors to
///   find the nearest existing one, canonicalizes that, and re-appends
///   the trailing components verbatim. This means symlinks under the
///   nearest existing ancestor *are* resolved, but the leaf names are
///   preserved as observed.
/// - Returns an error if the path is empty, contains a NUL byte, or has
///   no existing ancestor (which on Unix should not happen — `/` always
///   exists).
/// - Rejects paths whose string form contains any ASCII control character
///   other than horizontal tab; this guards against malformed log/event
///   data being silently emitted into a config file.
pub fn canonicalize_observed(path: &Path) -> io::Result<PathBuf> {
    validate(path)?;

    if path.try_exists()? {
        return std::fs::canonicalize(path);
    }

    let mut tail: Vec<&OsStr> = Vec::new();
    let mut current: &Path = path;
    loop {
        if let Some(name) = current.file_name() {
            tail.push(name);
        }
        let Some(parent) = current.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no existing ancestor for path: {}", path.display()),
            ));
        };
        if parent.as_os_str().is_empty() {
            let base = std::fs::canonicalize(std::env::current_dir()?)?;
            return Ok(append_tail(base, &tail));
        }
        if parent.try_exists()? {
            let base = std::fs::canonicalize(parent)?;
            return Ok(append_tail(base, &tail));
        }
        current = parent;
    }
}

fn append_tail(mut base: PathBuf, tail: &[&OsStr]) -> PathBuf {
    for name in tail.iter().rev() {
        base.push(name);
    }
    base
}

fn validate(path: &Path) -> io::Result<()> {
    let bytes = os_str_bytes(path.as_os_str());
    if bytes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "path is empty"));
    }
    for &b in bytes {
        if b == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains NUL byte",
            ));
        }
        let is_control = b < 0x20 || b == 0x7f;
        if is_control && b != b'\t' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("path contains control byte 0x{b:02x}"),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn os_str_bytes(s: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes()
}

#[cfg(not(unix))]
fn os_str_bytes(s: &OsStr) -> &[u8] {
    s.to_str().map(str::as_bytes).unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn existing_file_canonicalizes() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("hello.txt");
        fs::write(&file, b"x").unwrap();
        let got = canonicalize_observed(&file).unwrap();
        assert_eq!(got, fs::canonicalize(&file).unwrap());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_resolves_tmp_to_private_tmp() {
        let tmp = tempfile::tempdir_in("/tmp").unwrap();
        let file = tmp.path().join("a.txt");
        fs::write(&file, b"x").unwrap();
        let got = canonicalize_observed(&file).unwrap();
        assert!(
            got.starts_with("/private/tmp"),
            "expected /private/tmp prefix, got {got:?}"
        );
    }

    #[test]
    fn nonexisting_under_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("does/not/exist.txt");
        let got = canonicalize_observed(&target).unwrap();
        let canon_tmp = fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(got, canon_tmp.join("does/not/exist.txt"));
    }

    #[test]
    #[cfg(unix)]
    fn symlinked_parent_is_resolved_for_nonexisting_leaf() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let target = link.join("nonexistent.txt");
        let got = canonicalize_observed(&target).unwrap();
        let canon_real = fs::canonicalize(&real).unwrap();
        assert_eq!(got, canon_real.join("nonexistent.txt"));
    }

    #[test]
    fn dot_and_dotdot_components_resolve_in_existing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let file = sub.join("f.txt");
        fs::write(&file, b"x").unwrap();

        let weird = sub.join("./../sub/./f.txt");
        let got = canonicalize_observed(&weird).unwrap();
        assert_eq!(got, fs::canonicalize(&file).unwrap());
    }

    #[test]
    fn empty_path_errors() {
        let err = canonicalize_observed(Path::new("")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn control_character_errors() {
        let err = canonicalize_observed(Path::new("/tmp/foo\x01bar")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    #[cfg(unix)]
    fn nul_byte_errors() {
        use std::os::unix::ffi::OsStrExt;
        let bad = OsStr::from_bytes(b"/tmp/foo\0bar");
        let err = canonicalize_observed(Path::new(bad)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn idempotent_on_existing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("f.txt");
        fs::write(&file, b"x").unwrap();
        let once = canonicalize_observed(&file).unwrap();
        let twice = canonicalize_observed(&once).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn idempotent_on_nonexisting_path() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("a/b/c.txt");
        let once = canonicalize_observed(&target).unwrap();
        let twice = canonicalize_observed(&once).unwrap();
        assert_eq!(once, twice);
    }
}
