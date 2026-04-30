use std::path::{Path, PathBuf};

use crate::paths::{path_candidates, push_unique, push_with_ancestors, stdio_tty_paths};
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
        self.collect_read_ancestors(path);
        self.collect_into(path, |s| &mut s.read_paths);
    }
    fn add_read_dir(&mut self, path: &Path) {
        self.collect_read_ancestors(path);
        self.collect_into(path, |s| &mut s.read_dirs);
    }
    fn add_write(&mut self, path: &Path) {
        self.collect_read_ancestors(path);
        self.collect_into(path, |s| &mut s.write_paths);
    }
    fn add_write_dir(&mut self, path: &Path) {
        self.collect_read_ancestors(path);
        self.collect_into(path, |s| &mut s.write_dirs);
    }
    fn add_exec(&mut self, path: &Path) {
        self.collect_read_ancestors(path);
        self.collect_into(path, |s| &mut s.executable_paths);
    }
    fn add_exec_dir(&mut self, path: &Path) {
        self.collect_read_ancestors(path);
        self.collect_into(path, |s| &mut s.executable_dirs);
    }
    fn add_ioctl(&mut self, path: &Path) {
        self.collect_into(path, |s| &mut s.ioctl_paths);
    }

    fn collect_into(&mut self, path: &Path, target: fn(&mut Self) -> &mut Vec<PathBuf>) {
        for candidate in path_candidates(path) {
            push_unique(target(self), candidate);
        }
    }

    fn collect_read_ancestors(&mut self, path: &Path) {
        for candidate in path_candidates(path) {
            push_with_ancestors(&mut self.read_paths, &candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::SandboxSpec;

    use super::PathSets;

    fn path_sets_for(spec: &SandboxSpec) -> PathSets {
        PathSets::from_inputs(Path::new("/bin/sh"), spec, Path::new("/tmp/lockin-private"))
    }

    fn assert_read_paths_contain(paths: &PathSets, expected: &[&str]) {
        for expected in expected {
            assert!(
                paths.read_paths.contains(&PathBuf::from(expected)),
                "read_paths missing {expected}; got {:?}",
                paths.read_paths
            );
        }
    }

    #[test]
    fn exec_path_adds_leaf_and_ancestors_to_read_paths() {
        let spec = SandboxSpec {
            exec_paths: vec![PathBuf::from("/usr/bin/foo")],
            ..SandboxSpec::default()
        };

        let paths = path_sets_for(&spec);

        assert_read_paths_contain(&paths, &["/usr/bin/foo", "/usr/bin", "/usr", "/"]);
        assert!(paths
            .executable_paths
            .contains(&PathBuf::from("/usr/bin/foo")));
    }

    #[test]
    fn each_filesystem_category_adds_ancestors_to_read_paths() {
        let cases = [
            SandboxSpec {
                exec_dirs: vec![PathBuf::from("/opt/tool/bin")],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                read_paths: vec![PathBuf::from("/var/data/input.txt")],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                read_dirs: vec![PathBuf::from("/var/data")],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                write_paths: vec![PathBuf::from("/var/output/result.txt")],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                write_dirs: vec![PathBuf::from("/var/output")],
                ..SandboxSpec::default()
            },
        ];
        let expected = [
            &["/opt/tool/bin", "/opt/tool", "/opt", "/"][..],
            &["/var/data/input.txt", "/var/data", "/var", "/"][..],
            &["/var/data", "/var", "/"][..],
            &["/var/output/result.txt", "/var/output", "/var", "/"][..],
            &["/var/output", "/var", "/"][..],
        ];

        for (spec, expected) in cases.iter().zip(expected) {
            let paths = path_sets_for(spec);
            assert_read_paths_contain(&paths, expected);
        }
    }

    #[test]
    fn shared_ancestors_are_deduplicated_in_read_paths() {
        let spec = SandboxSpec {
            exec_paths: vec![PathBuf::from("/usr/bin/foo"), PathBuf::from("/usr/bin/bar")],
            ..SandboxSpec::default()
        };

        let paths = path_sets_for(&spec);

        for shared in ["/usr/bin", "/usr", "/"] {
            let count = paths
                .read_paths
                .iter()
                .filter(|path| path.as_path() == Path::new(shared))
                .count();
            assert_eq!(
                count, 1,
                "{shared} should appear once in {:?}",
                paths.read_paths
            );
        }
    }
}
