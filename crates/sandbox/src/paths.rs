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

/// Resolves each stdio fd (0, 1, 2) through `fd_dir` (e.g.
/// `/dev/fd` on macOS, `/proc/self/fd` on Linux) and returns the
/// canonical `/dev/*` targets. Used to build the interactive-tty
/// allowlist so the sandbox permits ioctl on the real tty device.
pub(crate) fn stdio_tty_paths(fd_dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();

    for fd in [0, 1, 2] {
        let fd_path = PathBuf::from(format!("{fd_dir}/{fd}"));
        if let Ok(target) = std::fs::canonicalize(&fd_path) {
            if target.starts_with("/dev/") {
                push_unique(&mut out, target);
            }
        }
    }

    out
}

/// Appends `path` to `paths` if it isn't already present.
/// Linear scan is fine: path lists are small (tens of entries).
pub(crate) fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
}
