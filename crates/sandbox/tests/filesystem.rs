//! Filesystem access contract: reads, stats, and writes are scoped
//! to explicit paths. The host's tmp is blocked; the sandbox's
//! private tmpdir is writable.

mod common;

use common::{run_probe, TestDir};

// ── read scoping ─────────────────────────────────────────────

#[test]
fn read_allowlist_grants_named_file() {
    let temp = TestDir::new("read-grant");
    let allowed = temp.join("allowed.txt");
    std::fs::write(&allowed, b"ok").expect("write fixture");

    assert!(run_probe(
        common::sandbox_builder().read_only_path(allowed.clone()),
        &["can-read", &allowed.display().to_string()]
    ));
}

#[test]
fn read_allowlist_does_not_grant_siblings() {
    let temp = TestDir::new("read-sibling");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");
    std::fs::write(&allowed, b"allowed").expect("write allowed");
    std::fs::write(&sibling, b"sibling").expect("write sibling");

    assert!(!run_probe(
        common::sandbox_builder().read_only_path(allowed),
        &["can-read", &sibling.display().to_string()]
    ));
}

// ── directory read scoping ────────────────────────────────────

#[test]
fn read_only_directory_grants_access_to_children() {
    let temp = TestDir::new("read-dir");
    let child_file = temp.join("nested.txt");
    std::fs::write(&child_file, b"nested content").expect("write nested file");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_only_path(dir),
        &["can-read", &child_file.display().to_string()]
    ));
}

// ── read-only enforcement ────────────────────────────────────

#[test]
fn read_only_path_blocks_writes() {
    let temp = TestDir::new("ro-enforce");
    let target = temp.join("readonly.txt");
    std::fs::write(&target, b"seed").expect("seed target");

    assert!(
        run_probe(
            common::sandbox_builder().read_only_path(target.clone()),
            &["can-read", &target.display().to_string()]
        ),
        "read should succeed on read_only_path"
    );
    assert!(
        !run_probe(
            common::sandbox_builder().read_only_path(target.clone()),
            &["can-write", &target.display().to_string()]
        ),
        "write should fail on read_only_path"
    );
}

// ── stat scoping ─────────────────────────────────────────────

#[test]
fn stat_allowlist_does_not_grant_siblings() {
    let temp = TestDir::new("stat-sibling");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");
    std::fs::write(&allowed, b"allowed").expect("write allowed");
    std::fs::write(&sibling, b"sibling").expect("write sibling");

    assert!(run_probe(
        common::sandbox_builder().read_only_path(allowed.clone()),
        &["can-stat", &allowed.display().to_string()]
    ));
    assert!(!run_probe(
        common::sandbox_builder().read_only_path(allowed),
        &["can-stat", &sibling.display().to_string()]
    ));
}

// ── write scoping ────────────────────────────────────────────

#[test]
fn write_allowlist_is_scoped_to_explicit_rw_paths() {
    let temp = TestDir::new("write-scope");
    let allowed = temp.join("ok.txt");
    let denied = temp.join("nope.txt");
    std::fs::write(&allowed, b"seed").expect("seed allowed");
    std::fs::write(&denied, b"seed").expect("seed denied");

    assert!(run_probe(
        common::sandbox_builder().read_write_path(allowed.clone()),
        &["can-write", &allowed.display().to_string()]
    ));
    assert!(!run_probe(
        common::sandbox_builder().read_write_path(allowed),
        &["can-write", &denied.display().to_string()]
    ));
}

// ── tmp scoping ──────────────────────────────────────────────

#[test]
fn host_tmp_is_not_writable_without_explicit_allowlist() {
    let temp = TestDir::new("host-tmp");
    let host_file = temp.join("host-target.txt");
    std::fs::write(&host_file, b"seed").expect("seed host file");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-write", &host_file.display().to_string()]
    ));
}

#[test]
fn private_tmpdir_is_writable() {
    assert!(run_probe(common::sandbox_builder(), &["can-write-temp"]));
}
