use std::path::{Path, PathBuf};

use crate::paths::{path_candidates, push_unique, stdio_tty_paths};
use crate::SandboxSpec;

#[derive(Debug, Default)]
pub(super) struct PathSets {
    pub(super) executable_paths: Vec<PathBuf>,
    pub(super) executable_dirs: Vec<PathBuf>,
    pub(super) read_paths: Vec<PathBuf>,
    pub(super) read_dirs: Vec<PathBuf>,
    pub(super) write_paths: Vec<PathBuf>,
    pub(super) write_dirs: Vec<PathBuf>,
    pub(super) ioctl_paths: Vec<PathBuf>,
    pub(super) traversal_paths: Vec<PathBuf>,
}

impl PathSets {
    pub(super) fn from_inputs(program: &Path, spec: &SandboxSpec, private_tmp: &Path) -> Self {
        let mut paths = Self::default();

        for candidate in path_candidates(program) {
            push_unique(&mut paths.executable_paths, candidate);
        }
        paths.add_read(program);

        for path in &spec.read_paths {
            paths.add_read(path);
        }
        for dir in &spec.read_dirs {
            paths.add_read_dir(dir);
        }

        for path in &spec.write_paths {
            paths.add_write(path);
        }
        for dir in &spec.write_dirs {
            paths.add_write_dir(dir);
        }

        for path in &spec.exec_paths {
            paths.add_exec(path);
        }
        for dir in &spec.exec_dirs {
            paths.add_exec_dir(dir);
        }

        if spec.allow_interactive_tty {
            for tty in stdio_tty_paths("/dev/fd") {
                paths.add_read(&tty);
                paths.add_write(&tty);
                paths.add_ioctl(&tty);
            }
            paths.add_read(Path::new("/dev/tty"));
            paths.add_write(Path::new("/dev/tty"));
            paths.add_ioctl(Path::new("/dev/tty"));
        }

        paths.add_write_dir(private_tmp);

        paths
    }

    fn add_read(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.read_paths);
    }
    fn add_read_dir(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.read_dirs);
    }
    fn add_write(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.write_paths);
    }
    fn add_write_dir(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.write_dirs);
    }
    fn add_exec(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.executable_paths);
        self.add_read(path);
    }
    fn add_exec_dir(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.executable_dirs);
        self.add_read_dir(path);
    }
    fn add_ioctl(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.ioctl_paths);
    }

    fn collect_into(&mut self, path: &Path, target: fn(&mut Self) -> &mut Vec<PathBuf>) {
        let candidates: Vec<_> = path_candidates(path);
        for candidate in candidates {
            push_unique(target(self), candidate.clone());
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
