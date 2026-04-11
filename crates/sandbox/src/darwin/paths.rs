use std::path::{Path, PathBuf};

use crate::paths::{path_candidates, push_unique, stdio_tty_paths};
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

        for candidate in path_candidates(program) {
            push_unique(&mut paths.executable_paths, candidate);
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

        for dir in &spec.library_paths {
            paths.add_read_only(dir);
        }

        if spec.allow_interactive_tty {
            for tty in stdio_tty_paths("/dev/fd") {
                paths.add_ioctl(&tty);
            }
            paths.add_ioctl(Path::new("/dev/tty"));
        }

        paths.add_read_write(private_tmp);

        paths
    }

    fn add_read_only(&mut self, path: &Path) {
        for candidate in path_candidates(path) {
            push_unique(&mut self.read_only_paths, candidate.clone());
            self.add_traversal_ancestors(&candidate);
        }
    }

    fn add_read_write(&mut self, path: &Path) {
        for candidate in path_candidates(path) {
            push_unique(&mut self.read_write_paths, candidate.clone());
            self.add_traversal_ancestors(&candidate);
        }
    }

    fn add_ioctl(&mut self, path: &Path) {
        for candidate in path_candidates(path) {
            push_unique(&mut self.ioctl_paths, candidate.clone());
            self.add_traversal_ancestors(&candidate);
        }
    }

    fn add_traversal_ancestors(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            for ancestor in parent.ancestors() {
                if ancestor == Path::new("/") {
                    break;
                }
                push_unique(&mut self.traversal_paths, ancestor.to_path_buf());
            }
        }
    }
}
