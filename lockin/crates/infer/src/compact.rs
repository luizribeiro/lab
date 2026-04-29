//! Convert observed events into a structured policy ready for TOML emission.
//!
//! This is platform-independent. Backends produce `Vec<InferEvent>`; the
//! compactor turns that into the canonical sets of read paths/dirs, write
//! paths/dirs, and exec paths a `lockin.toml` would express.
//!
//! Pipeline:
//! 1. Classify each event into raw read/write/exec contributions.
//! 2. Collapse reads under known immutable system prefixes (e.g. `/usr/lib`,
//!    `/nix/store`) into a single `read_dirs` entry. Avoids hundreds of
//!    dynamic-linker library entries in generated configs.
//! 3. No capability implication is performed here — both lockin enforcement
//!    backends synthesize traversal/read coverage from explicit leaf entries
//!    (see `sandbox/src/linux.rs` and `sandbox/src/darwin/paths.rs`).
//!
//! Note: `FsOp::Stat` is promoted to a read. Real metadata-only support
//! would require a separate schema field; until then this is a deliberate
//! conservative overgrant, surfaced in the generated TOML's header.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::event::{FsOp, InferEvent};

/// Compacted, deduped, sorted policy derived from a batch of events.
///
/// Field semantics map onto the `[filesystem]` schema in lockin.toml:
/// the union of `read_paths` + `read_dirs` is the read capability,
/// likewise for write and exec.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferredPolicy {
    pub read_paths: BTreeSet<PathBuf>,
    pub read_dirs: BTreeSet<PathBuf>,
    pub write_paths: BTreeSet<PathBuf>,
    pub write_dirs: BTreeSet<PathBuf>,
    pub exec_paths: BTreeSet<PathBuf>,
}

/// Immutable system trees on supported platforms. Reads under any of
/// these collapse to a single `read_dirs` entry; collapsing avoids
/// hundreds of dynamic-linker / shared-library entries in generated
/// configs and keeps the output reviewable. Listed for both Linux and
/// Darwin; entries from one platform never match paths from the other,
/// so a unified list is harmless.
const SYSTEM_READ_PREFIXES: &[&str] = &[
    // Linux
    "/usr/lib",
    "/usr/lib64",
    "/lib",
    "/lib64",
    "/usr/share",
    "/nix/store",
    "/etc/ssl",
    "/etc/ca-certificates",
    // Darwin
    "/System",
    "/Library/Apple",
];

/// Convert events to a compacted policy.
pub fn compact(events: &[InferEvent]) -> InferredPolicy {
    let mut policy = InferredPolicy::default();

    for event in events {
        match event {
            InferEvent::Fs { op, path } => match op {
                FsOp::Read | FsOp::ReadDir | FsOp::Stat => {
                    add_read(path.clone(), &mut policy);
                }
                FsOp::Write => {
                    policy.write_paths.insert(path.clone());
                }
                FsOp::Create => {
                    policy.write_paths.insert(path.clone());
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() {
                            policy.write_dirs.insert(parent.to_path_buf());
                        }
                    }
                }
                FsOp::Delete => {
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() {
                            policy.write_dirs.insert(parent.to_path_buf());
                        }
                    }
                }
            },
            InferEvent::Exec { path } => {
                policy.exec_paths.insert(path.clone());
            }
            InferEvent::Unsupported { .. } => {}
        }
    }

    policy
}

fn add_read(path: PathBuf, policy: &mut InferredPolicy) {
    if policy.read_dirs.iter().any(|d| path.starts_with(d)) {
        return;
    }
    if let Some(prefix) = matching_system_prefix(&path) {
        policy.read_dirs.insert(PathBuf::from(prefix));
        return;
    }
    policy.read_paths.insert(path);
}

fn matching_system_prefix(path: &Path) -> Option<&'static str> {
    SYSTEM_READ_PREFIXES
        .iter()
        .copied()
        .find(|prefix| path.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs(op: FsOp, p: &str) -> InferEvent {
        InferEvent::Fs {
            op,
            path: PathBuf::from(p),
        }
    }

    fn exec(p: &str) -> InferEvent {
        InferEvent::Exec {
            path: PathBuf::from(p),
        }
    }

    #[test]
    fn empty_input_produces_empty_policy() {
        let policy = compact(&[]);
        assert_eq!(policy, InferredPolicy::default());
    }

    #[test]
    fn read_of_etc_hosts_lands_in_read_paths() {
        let policy = compact(&[fs(FsOp::Read, "/etc/hosts")]);
        assert!(policy.read_paths.contains(Path::new("/etc/hosts")));
        assert!(policy.read_dirs.is_empty());
    }

    #[test]
    fn read_under_usr_lib_collapses_to_read_dirs() {
        let policy = compact(&[fs(FsOp::Read, "/usr/lib/foo.so")]);
        assert!(policy.read_dirs.contains(Path::new("/usr/lib")));
        assert!(!policy.read_paths.contains(Path::new("/usr/lib/foo.so")));
    }

    #[test]
    fn read_under_nix_store_collapses() {
        let policy = compact(&[fs(FsOp::Read, "/nix/store/abc-glibc/lib/libc.so.6")]);
        assert!(policy.read_dirs.contains(Path::new("/nix/store")));
        assert!(policy.read_paths.is_empty());
    }

    #[test]
    fn many_reads_under_same_prefix_collapse_to_one() {
        let policy = compact(&[
            fs(FsOp::Read, "/usr/lib/a.so"),
            fs(FsOp::Read, "/usr/lib/b.so"),
            fs(FsOp::Read, "/usr/lib/sub/c.so"),
        ]);
        assert_eq!(policy.read_dirs.len(), 1);
        assert!(policy.read_dirs.contains(Path::new("/usr/lib")));
    }

    #[test]
    fn stat_promotes_to_read() {
        let policy = compact(&[fs(FsOp::Stat, "/etc/hosts")]);
        assert!(policy.read_paths.contains(Path::new("/etc/hosts")));
    }

    #[test]
    fn readdir_promotes_to_read() {
        let policy = compact(&[fs(FsOp::ReadDir, "/etc/hosts")]);
        assert!(policy.read_paths.contains(Path::new("/etc/hosts")));
    }

    #[test]
    fn create_emits_file_write_and_dir_write_for_parent() {
        let policy = compact(&[fs(FsOp::Create, "/tmp/work/out.txt")]);
        assert!(policy.write_paths.contains(Path::new("/tmp/work/out.txt")));
        assert!(policy.write_dirs.contains(Path::new("/tmp/work")));
    }

    #[test]
    fn delete_emits_dir_write_only() {
        let policy = compact(&[fs(FsOp::Delete, "/tmp/work/out.txt")]);
        assert!(policy.write_dirs.contains(Path::new("/tmp/work")));
        assert!(!policy.write_paths.contains(Path::new("/tmp/work/out.txt")));
        assert!(policy.write_paths.is_empty());
    }

    #[test]
    fn exec_implication_does_not_pollute_read_paths() {
        let policy = compact(&[exec("/usr/bin/ls")]);
        assert!(policy.exec_paths.contains(Path::new("/usr/bin/ls")));
        assert!(
            policy.read_paths.is_empty(),
            "exec must not imply read_paths entry: {:?}",
            policy.read_paths,
        );
        assert!(policy.read_dirs.is_empty());
    }

    #[test]
    fn exec_under_nix_store_does_not_add_read_paths() {
        let policy = compact(&[exec("/nix/store/abc/bin/foo")]);
        assert!(policy
            .exec_paths
            .contains(Path::new("/nix/store/abc/bin/foo")));
        assert!(
            policy.read_paths.is_empty(),
            "exec alone must not populate read_paths: {:?}",
            policy.read_paths,
        );
        assert!(
            policy.read_dirs.is_empty(),
            "exec alone must not populate read_dirs: {:?}",
            policy.read_dirs,
        );
    }

    #[test]
    fn write_does_not_add_parent_to_read_paths() {
        let policy = compact(&[fs(FsOp::Write, "/home/u/proj/out.txt")]);
        assert!(policy
            .write_paths
            .contains(Path::new("/home/u/proj/out.txt")));
        assert!(
            policy.write_dirs.is_empty(),
            "Write op alone must not add parent to write_dirs: {:?}",
            policy.write_dirs,
        );
        assert!(
            policy.read_paths.is_empty(),
            "Write op alone must not add parent to read_paths: {:?}",
            policy.read_paths,
        );
    }

    #[test]
    fn write_under_system_prefix_does_not_collapse_parent_into_read_dirs() {
        let policy = compact(&[fs(FsOp::Write, "/usr/share/data/foo.bin")]);
        assert!(policy
            .write_paths
            .contains(Path::new("/usr/share/data/foo.bin")));
        assert!(
            policy.read_dirs.is_empty(),
            "Write op alone must not populate read_dirs: {:?}",
            policy.read_dirs,
        );
        assert!(policy.read_paths.is_empty());
    }

    #[test]
    fn dedup_repeated_reads() {
        let events: Vec<InferEvent> = (0..100).map(|_| fs(FsOp::Read, "/etc/hosts")).collect();
        let policy = compact(&events);
        assert_eq!(policy.read_paths.len(), 1);
    }

    #[test]
    fn ordering_is_deterministic_regardless_of_input_order() {
        let mut a = vec![
            fs(FsOp::Read, "/etc/hosts"),
            fs(FsOp::Read, "/etc/resolv.conf"),
            fs(FsOp::Read, "/usr/lib/x.so"),
            exec("/usr/bin/ls"),
        ];
        let mut b = a.clone();
        a.reverse();
        b.rotate_left(2);
        assert_eq!(compact(&a), compact(&b));
        // Iteration order of BTreeSet is sorted.
        let policy = compact(&a);
        let collected: Vec<&PathBuf> = policy.read_paths.iter().collect();
        let mut sorted = collected.clone();
        sorted.sort();
        assert_eq!(collected, sorted);
    }

    #[test]
    fn unsupported_event_ignored() {
        let policy = compact(&[InferEvent::Unsupported {
            backend: "test",
            raw: "raw".into(),
            reason: "no schema".into(),
        }]);
        assert_eq!(policy, InferredPolicy::default());
    }

    #[test]
    fn etc_ssl_and_etc_ca_certificates_collapse() {
        let policy = compact(&[
            fs(FsOp::Read, "/etc/ssl/cert.pem"),
            fs(
                FsOp::Read,
                "/etc/ca-certificates/extracted/tls-ca-bundle.pem",
            ),
        ]);
        assert!(policy.read_dirs.contains(Path::new("/etc/ssl")));
        assert!(policy.read_dirs.contains(Path::new("/etc/ca-certificates")));
        assert!(policy.read_paths.is_empty());
    }

    #[test]
    fn darwin_system_prefix_collapses() {
        let policy = compact(&[fs(
            FsOp::Read,
            "/System/Library/dyld/dyld_shared_cache_arm64e",
        )]);
        assert!(policy.read_dirs.contains(Path::new("/System")));
    }

    #[test]
    fn write_into_system_dir_stays_at_file_level() {
        let policy = compact(&[fs(FsOp::Write, "/usr/lib/oddly-writable.so")]);
        assert!(
            policy
                .write_paths
                .contains(Path::new("/usr/lib/oddly-writable.so")),
            "writes to system dirs must stay at file level: {:?}",
            policy.write_paths,
        );
        assert!(
            policy.write_dirs.is_empty(),
            "writes never collapse to dir-level grants from system prefixes: {:?}",
            policy.write_dirs,
        );
    }
}
