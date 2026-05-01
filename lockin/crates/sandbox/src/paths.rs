//! Shared path utilities used by both the darwin and linux sandbox
//! backends to normalize, deduplicate, and resolve paths before
//! encoding them into platform-specific policy rules.

use std::path::{Path, PathBuf};

/// Returns the literal path (resolved to absolute if relative) plus
/// its `fs::canonicalize` form if that differs. The sandbox policy
/// must list both because the kernel may resolve symlinks at
/// `open(2)` time and check the post-resolution path against the
/// policy.
pub(crate) fn path_candidates(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };

    push_unique(&mut out, absolute.clone());

    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        push_unique(&mut out, canonical);
    }

    out
}

/// Resolves each stdio fd (0, 1, 2) and returns the canonical
/// `/dev/*` device path for any that is a tty. On macOS,
/// `realpath("/dev/fd/0")` is a no-op (the `fdesc` filesystem does
/// not resolve to the underlying tty), so we prefer `ttyname(3)` —
/// it returns the actual `/dev/ttys*` path that Seatbelt's
/// `file-ioctl` rules match against. The `fd_dir` fallback handles
/// Linux (`/proc/self/fd`), where canonicalization works.
pub(crate) fn stdio_tty_paths(fd_dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();

    for fd in [0, 1, 2] {
        if let Some(name) = tty_name_of_fd(fd) {
            if name.starts_with("/dev/") {
                push_unique(&mut out, PathBuf::from(name));
                continue;
            }
        }
        let fd_path = PathBuf::from(format!("{fd_dir}/{fd}"));
        if let Ok(target) = std::fs::canonicalize(&fd_path) {
            if target.starts_with("/dev/") {
                push_unique(&mut out, target);
            }
        }
    }

    out
}

/// Returns the path of the terminal associated with `fd` if it is a
/// tty, else `None`. Wraps `ttyname(3)` so the result is always a
/// real device path (e.g. `/dev/ttys028`) — `realpath("/dev/fd/0")`
/// on macOS does not resolve to the underlying device.
fn tty_name_of_fd(fd: i32) -> Option<String> {
    if unsafe { libc::isatty(fd) } != 1 {
        return None;
    }
    let ptr = unsafe { libc::ttyname(fd) };
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Splits the strict ancestors of `path` (i.e. excluding the leaf
/// itself) into the components the kernel only needs to look up
/// versus the components it needs to read the bytes of.
///
/// A directory just needs lookup/stat to traverse — granting
/// `file-read-data` on it would also grant `readdir`, leaking the
/// directory's listing. A symlink, however, must be readable as data
/// for the kernel to follow it. So we lstat each ancestor and place
/// it in the appropriate bucket; ancestors that don't exist (or fail
/// to stat) fall through to traversal, which is the safe default
/// since a non-symlink rule grants strictly less than read-data.
///
/// For each ancestor we also include the same component rebuilt
/// against its parent's canonical form. This covers cases like macOS
/// `/var → /private/var`: when the kernel resolves `/var/.../link`
/// it can hand the policy check the post-resolution path
/// `/private/var/.../link`, even though the caller only ever named
/// `/var`.
///
/// The returned vectors are deduplicated and ordered leaf-to-root.
pub(crate) fn ancestor_sets(path: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut traversal = Vec::new();
    let mut symlink = Vec::new();
    for ancestor in path.ancestors().skip(1) {
        classify_ancestor(ancestor, &mut traversal, &mut symlink);
        if let (Some(parent), Some(file_name)) = (ancestor.parent(), ancestor.file_name()) {
            if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
                let rebuilt = canonical_parent.join(file_name);
                if rebuilt != ancestor {
                    classify_ancestor(&rebuilt, &mut traversal, &mut symlink);
                }
            }
        }
    }
    (traversal, symlink)
}

fn classify_ancestor(path: &Path, traversal: &mut Vec<PathBuf>, symlink: &mut Vec<PathBuf>) {
    let target = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => symlink,
        _ => traversal,
    };
    push_unique(target, path.to_path_buf());
}

/// Appends `path` to `paths` if it isn't already present.
/// Linear scan is fine: path lists are small (tens of entries).
pub(crate) fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    /// Allocates a pty pair and runs `f(slave_fd)` so tests can
    /// exercise the tty branch of `stdio_tty_paths` without needing
    /// the test process itself to run under a tty.
    fn with_pty_slave<F: FnOnce(i32)>(f: F) {
        let mut master_fd: libc::c_int = 0;
        let mut slave_fd: libc::c_int = 0;
        let rc = unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0, "openpty: {}", std::io::Error::last_os_error());
        let _master = unsafe { OwnedFd::from_raw_fd(master_fd) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave_fd) };
        f(slave.as_raw_fd());
    }

    #[test]
    fn stdio_tty_paths_returns_real_device_for_pty_slave() {
        with_pty_slave(|slave| {
            // Redirect stdin (fd 0) to the pty slave for the duration
            // of the call so `stdio_tty_paths` sees a tty there. Save
            // and restore the original to avoid polluting test
            // harness IO.
            let saved = unsafe { libc::dup(0) };
            assert!(saved >= 0);
            assert!(unsafe { libc::dup2(slave, 0) } >= 0);

            let paths = stdio_tty_paths("/dev/fd");

            assert!(unsafe { libc::dup2(saved, 0) } >= 0);
            unsafe { libc::close(saved) };

            assert!(
                paths.iter().any(|p| {
                    let s = p.to_string_lossy();
                    // Linux pty: /dev/pts/N. macOS pty: /dev/ttys*.
                    // What we want to rule out is the unresolved
                    // `/dev/fd/0` form, which would not match a
                    // Seatbelt `(literal "/dev/ttys*")` rule.
                    s.starts_with("/dev/pts/") || s.starts_with("/dev/ttys")
                }),
                "expected a real tty device path, got: {paths:?}"
            );
            assert!(
                !paths.iter().any(|p| p.to_string_lossy() == "/dev/fd/0"),
                "/dev/fd/0 must not appear (Seatbelt rules match the device path, not the fdesc symlink): {paths:?}"
            );
        });
    }
}
