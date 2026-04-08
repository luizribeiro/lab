use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lddtree::DependencyAnalyzer;

/// Returns the set of directories the dynamic linker will search to load
/// `program` and its transitive dependencies.
///
/// The result includes the parent directory of the interpreter and of every
/// library resolved by [`lddtree`] from the binary's ELF / Mach-O headers.
/// Returning directories rather than individual files lets callers grant
/// coarse read+exec on each, which also permits `dlopen` of sibling libraries
/// in the same directory (NSS modules, locale data, ICU plugins, ...).
///
/// Returns an empty set if the binary cannot be read or parsed.
pub(crate) fn library_dirs(program: &Path) -> BTreeSet<PathBuf> {
    let mut dirs = BTreeSet::new();

    let analyzer = DependencyAnalyzer::new(PathBuf::from("/"));
    let Ok(tree) = analyzer.analyze(program) else {
        return dirs;
    };

    if let Some(interpreter) = tree.interpreter.as_deref() {
        insert_parent(&mut dirs, Path::new(interpreter));
    }

    for library in tree.libraries.values() {
        if let Some(realpath) = library.realpath.as_deref() {
            insert_parent(&mut dirs, realpath);
        }
    }

    dirs
}

fn insert_parent(dirs: &mut BTreeSet<PathBuf>, path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            dirs.insert(parent.to_path_buf());
        }
    }
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
}
