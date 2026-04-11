use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lddtree::DependencyAnalyzer;

/// Trusted directory prefixes under which dynamically-linked library
/// directories are allowed. Directories outside these roots are
/// rejected to prevent an attacker who controls the binary's ELF/Mach-O
/// headers from widening the sandbox policy via crafted PT_INTERP,
/// DT_RUNPATH, or DT_NEEDED entries.
#[cfg(target_os = "linux")]
const TRUSTED_LIB_PREFIXES: &[&str] = &[
    "/lib",
    "/lib64",
    "/usr/lib",
    "/usr/lib64",
    "/usr/local/lib",
    "/nix/store",
];

#[cfg(target_os = "macos")]
const TRUSTED_LIB_PREFIXES: &[&str] = &[
    "/usr/lib",
    "/Library",
    "/opt/homebrew/lib",
    "/opt/homebrew/Cellar",
    "/nix/store",
];

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!(
    "TRUSTED_LIB_PREFIXES is not defined for this platform; \
     add an appropriate allowlist of library directory roots"
);

/// Returns the set of directories the dynamic linker will search to load
/// `program` and its transitive dependencies.
///
/// The result includes the parent directory of the interpreter and of every
/// library resolved by [`lddtree`] from the binary's ELF / Mach-O headers.
/// Returning directories rather than individual files lets callers grant
/// coarse read+exec on each, which also permits `dlopen` of sibling libraries
/// in the same directory (NSS modules, locale data, ICU plugins, ...).
///
/// Only directories under [`TRUSTED_LIB_PREFIXES`] are returned.
///
/// Returns an empty set if the binary cannot be read or parsed.
pub(crate) fn library_dirs(program: &Path) -> BTreeSet<PathBuf> {
    let mut dirs = BTreeSet::new();

    let analyzer = DependencyAnalyzer::new(PathBuf::from("/"));
    let Ok(tree) = analyzer.analyze(program) else {
        return dirs;
    };

    if let Some(interpreter) = tree.interpreter.as_deref() {
        insert_trusted_parent(&mut dirs, Path::new(interpreter));
    }

    for library in tree.libraries.values() {
        if let Some(realpath) = library.realpath.as_deref() {
            insert_trusted_parent(&mut dirs, realpath);
        }
    }

    dirs
}

fn insert_trusted_parent(dirs: &mut BTreeSet<PathBuf>, path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    if !parent.is_absolute() {
        return;
    }
    if !is_under_trusted_prefix(parent) {
        return;
    }
    if !parent.is_dir() {
        return;
    }
    dirs.insert(parent.to_path_buf());
}

fn is_under_trusted_prefix(path: &Path) -> bool {
    TRUSTED_LIB_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_dirs_for_self_contains_interpreter_dir() {
        let current_exe = std::env::current_exe().expect("current_exe");
        let dirs = library_dirs(&current_exe);

        assert!(
            !dirs.is_empty(),
            "expected at least one library directory for {}",
            current_exe.display()
        );

        for dir in &dirs {
            assert!(
                dir.is_absolute(),
                "library dir is not absolute: {}",
                dir.display()
            );
        }
    }

    #[test]
    fn library_dirs_returns_empty_for_nonexistent_binary() {
        let dirs = library_dirs(Path::new("/nonexistent/binary-that-does-not-exist"));
        assert!(dirs.is_empty());
    }

    #[test]
    fn all_returned_dirs_are_under_trusted_prefixes() {
        let current_exe = std::env::current_exe().expect("current_exe");
        let dirs = library_dirs(&current_exe);

        for dir in &dirs {
            assert!(
                is_under_trusted_prefix(dir),
                "library dir {} is not under any trusted prefix",
                dir.display()
            );
        }
    }

    #[test]
    fn untrusted_prefix_is_rejected() {
        assert!(!is_under_trusted_prefix(Path::new("/home/victim/.ssh")));
        assert!(!is_under_trusted_prefix(Path::new("/tmp/evil")));
        assert!(!is_under_trusted_prefix(Path::new("/etc/shadow")));
    }

    #[test]
    fn trusted_prefix_is_accepted() {
        assert!(is_under_trusted_prefix(Path::new("/usr/lib")));
        assert!(is_under_trusted_prefix(Path::new(
            "/usr/lib/x86_64-linux-gnu"
        )));
        assert!(is_under_trusted_prefix(Path::new(
            "/nix/store/abc123-glibc/lib"
        )));

        #[cfg(target_os = "macos")]
        {
            assert!(is_under_trusted_prefix(Path::new("/Library/Frameworks")));
            assert!(is_under_trusted_prefix(Path::new(
                "/opt/homebrew/lib/libssl"
            )));
        }
    }

    #[test]
    fn relative_path_is_rejected() {
        let mut dirs = BTreeSet::new();
        insert_trusted_parent(&mut dirs, Path::new("relative/path/libfoo.so"));
        assert!(dirs.is_empty());
    }
}
