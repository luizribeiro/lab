use std::path::{Path, PathBuf};

use crate::paths::{ancestor_sets, path_candidates, push_unique, stdio_tty_paths};
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
        for candidate in path_candidates(path) {
            let (traversal, symlink) = ancestor_sets(&candidate);
            for ancestor in traversal {
                push_unique(&mut self.traversal_paths, ancestor);
            }
            for ancestor in symlink {
                push_unique(&mut self.read_paths, ancestor);
            }
            push_unique(target(self), candidate);
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

    fn assert_traversal_contains(paths: &PathSets, expected: &[&str]) {
        for expected in expected {
            assert!(
                paths.traversal_paths.contains(&PathBuf::from(expected)),
                "traversal_paths missing {expected}; got {:?}",
                paths.traversal_paths
            );
        }
    }

    fn assert_no_directory_listing(paths: &PathSets, ancestors: &[&str]) {
        for ancestor in ancestors {
            assert!(
                !paths.read_paths.contains(&PathBuf::from(ancestor)),
                "{ancestor} must not be in read_paths (would grant readdir); got {:?}",
                paths.read_paths
            );
        }
    }

    #[test]
    fn exec_path_ancestors_land_in_traversal_not_read_paths() {
        let spec = SandboxSpec {
            exec_paths: vec![PathBuf::from("/usr/bin/foo")],
            ..SandboxSpec::default()
        };

        let paths = path_sets_for(&spec);

        assert_traversal_contains(&paths, &["/usr/bin", "/usr", "/"]);
        assert_no_directory_listing(&paths, &["/usr/bin", "/usr", "/"]);
        assert!(paths
            .executable_paths
            .contains(&PathBuf::from("/usr/bin/foo")));
        assert!(paths.read_paths.contains(&PathBuf::from("/usr/bin/foo")));
    }

    #[test]
    fn each_filesystem_category_routes_plain_ancestors_to_traversal() {
        // Build a real on-disk structure so lstat returns deterministic
        // results. Synthetic absolute paths can collide with real
        // symlinks on the host (e.g. /var → /private/var on macOS).
        let base = tempfile::Builder::new()
            .prefix("lockin-category-traversal-test-")
            .tempdir()
            .expect("tempdir");
        let plain = base.path().join("plain");
        std::fs::create_dir_all(plain.join("nested")).expect("create nested");
        let leaf_file = plain.join("nested/file.txt");
        std::fs::write(&leaf_file, b"x").expect("create leaf file");
        let leaf_dir = plain.join("nested");

        let cases = [
            SandboxSpec {
                read_paths: vec![leaf_file.clone()],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                read_dirs: vec![leaf_dir.clone()],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                write_paths: vec![leaf_file.clone()],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                write_dirs: vec![leaf_dir.clone()],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                exec_paths: vec![leaf_file.clone()],
                ..SandboxSpec::default()
            },
            SandboxSpec {
                exec_dirs: vec![leaf_dir.clone()],
                ..SandboxSpec::default()
            },
        ];

        // `plain` is a real dir we just created; assert it lands in
        // traversal and never grants directory listing.
        for spec in &cases {
            let paths = path_sets_for(spec);
            assert_traversal_contains(&paths, &[plain.to_str().unwrap()]);
            assert_no_directory_listing(&paths, &[plain.to_str().unwrap()]);
        }
    }

    #[test]
    fn real_symlink_ancestor_is_promoted_to_read_paths() {
        let base = tempfile::Builder::new()
            .prefix("lockin-symlink-ancestor-test-")
            .tempdir()
            .expect("create test base dir");
        let target_dir = base.path().join("real");
        std::fs::create_dir_all(&target_dir).expect("create target dir");
        let link = base.path().join("link");
        std::os::unix::fs::symlink(&target_dir, &link).expect("create symlink");
        let probe = link.join("probe.txt");

        let spec = SandboxSpec {
            read_paths: vec![probe.clone()],
            ..SandboxSpec::default()
        };
        let paths = path_sets_for(&spec);

        assert!(
            paths.read_paths.contains(&link),
            "symlink ancestor {link:?} must be promoted to read_paths to allow resolution; got {:?}",
            paths.read_paths
        );
        assert!(
            !paths.traversal_paths.contains(&link),
            "symlink ancestor {link:?} must not appear in traversal_paths; got {:?}",
            paths.traversal_paths
        );
    }

    #[test]
    fn shared_ancestors_are_deduplicated() {
        let spec = SandboxSpec {
            exec_paths: vec![PathBuf::from("/usr/bin/foo"), PathBuf::from("/usr/bin/bar")],
            ..SandboxSpec::default()
        };

        let paths = path_sets_for(&spec);

        for shared in ["/usr/bin", "/usr", "/"] {
            let count = paths
                .traversal_paths
                .iter()
                .filter(|path| path.as_path() == Path::new(shared))
                .count();
            assert_eq!(
                count, 1,
                "{shared} should appear once in {:?}",
                paths.traversal_paths
            );
        }
    }
}
