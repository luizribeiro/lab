use std::path::{Path, PathBuf};

use crate::discover::library_dirs;
use crate::SandboxSpec;

#[derive(Debug, Default)]
pub(super) struct PathSets {
    pub(super) executable_paths: Vec<PathBuf>,
    pub(super) read_only_paths: Vec<PathBuf>,
    pub(super) read_write_paths: Vec<PathBuf>,
    pub(super) ioctl_paths: Vec<PathBuf>,
    pub(super) traversal_paths: Vec<PathBuf>,
}

impl PathSets {
    pub(super) fn from_inputs(program: &Path, spec: &SandboxSpec, private_tmp: &Path) -> Self {
        let mut paths = Self::default();

        for candidate in Self::path_candidates(program) {
            Self::push_unique(&mut paths.executable_paths, candidate);
        }
        paths.add_read_only(program);

        for path in &spec.read_only_paths {
            paths.add_read_only(path);
        }

        for path in &spec.read_write_paths {
            paths.add_read_write(path);
        }

        for path in &spec.ioctl_paths {
            paths.add_ioctl(path);
        }

        // Grant read on each directory the dynamic linker will search for
        // `program`. Covers the link-time closure as well as any runtime
        // `dlopen` of siblings in the same directory.
        for dir in library_dirs(program) {
            paths.add_read_only(&dir);
        }

        // Interactive terminal support for libkrun console handling.
        for tty in stdio_tty_paths() {
            paths.add_ioctl(&tty);
        }
        paths.add_ioctl(Path::new("/dev/tty"));

        paths.add_read_write(private_tmp);

        paths
    }

    fn add_read_only(&mut self, path: &Path) {
        for candidate in Self::path_candidates(path) {
            Self::push_unique(&mut self.read_only_paths, candidate.clone());
            self.add_traversal_ancestors(&candidate);
        }
    }

    fn add_read_write(&mut self, path: &Path) {
        for candidate in Self::path_candidates(path) {
            Self::push_unique(&mut self.read_write_paths, candidate.clone());
            self.add_traversal_ancestors(&candidate);
        }
    }

    fn add_ioctl(&mut self, path: &Path) {
        for candidate in Self::path_candidates(path) {
            Self::push_unique(&mut self.ioctl_paths, candidate.clone());
            self.add_traversal_ancestors(&candidate);
        }
    }

    fn path_candidates(path: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();

        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(path)
        } else {
            path.to_path_buf()
        };

        Self::push_unique(&mut out, absolute.clone());

        if let Ok(canonical) = std::fs::canonicalize(&absolute) {
            Self::push_unique(&mut out, canonical);
        }

        out
    }

    fn add_traversal_ancestors(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            for ancestor in parent.ancestors() {
                if ancestor == Path::new("/") {
                    break;
                }
                Self::push_unique(&mut self.traversal_paths, ancestor.to_path_buf());
            }
        }
    }

    fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
        if !paths.iter().any(|p| p == &path) {
            paths.push(path);
        }
    }
}

fn stdio_tty_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();

    for fd in [0, 1, 2] {
        let fd_path = PathBuf::from(format!("/dev/fd/{fd}"));
        if let Ok(target) = std::fs::canonicalize(&fd_path) {
            if target.starts_with("/dev/") {
                PathSets::push_unique(&mut out, target);
            }
        }
    }

    out
}
