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
        common::sandbox_builder().read_only_dir(dir),
        &["can-read", &child_file.display().to_string()]
    ));
}

// ── directory write scoping ──────────────────────────────────

#[test]
fn read_write_directory_grants_write_to_children() {
    let temp = TestDir::new("write-dir");
    let child_file = temp.join("writable.txt");
    std::fs::write(&child_file, b"seed").expect("seed child file");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_write_dir(dir),
        &["can-write", &child_file.display().to_string()]
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

// ── readdir scoping ──────────────────────────────────────────

#[test]
fn read_only_directory_grants_readdir() {
    let temp = TestDir::new("readdir-ro");
    std::fs::write(temp.join("a.txt"), b"a").expect("seed a");
    std::fs::write(temp.join("b.txt"), b"b").expect("seed b");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_only_dir(dir.clone()),
        &["can-readdir", &dir.display().to_string()]
    ));
}

#[test]
fn read_write_directory_grants_readdir() {
    let temp = TestDir::new("readdir-rw");
    std::fs::write(temp.join("a.txt"), b"a").expect("seed a");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_write_dir(dir.clone()),
        &["can-readdir", &dir.display().to_string()]
    ));
}

#[test]
fn readdir_recurses_into_subdirectories() {
    let temp = TestDir::new("readdir-subdir");
    let subdir = temp.join("sub");
    std::fs::create_dir(&subdir).expect("mkdir sub");
    std::fs::write(subdir.join("nested.txt"), b"n").expect("seed nested");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_only_dir(dir),
        &["can-readdir", &subdir.display().to_string()]
    ));
}

// ── mkdir scoping ────────────────────────────────────────────

#[test]
fn read_write_directory_grants_mkdir() {
    let temp = TestDir::new("mkdir-rw");
    let new_dir = temp.join("created");

    assert!(run_probe(
        common::sandbox_builder().read_write_dir(temp.join("")),
        &["can-mkdir", &new_dir.display().to_string()]
    ));
    assert!(
        new_dir.is_dir(),
        "probe reported success but directory was not created on disk"
    );
}

#[test]
fn read_only_directory_does_not_grant_mkdir() {
    let temp = TestDir::new("mkdir-ro");
    let new_dir = temp.join("blocked");

    assert!(!run_probe(
        common::sandbox_builder().read_only_dir(temp.join("")),
        &["can-mkdir", &new_dir.display().to_string()]
    ));
}

#[test]
fn mkdir_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("mkdir-deny");
    let new_dir = temp.join("blocked");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-mkdir", &new_dir.display().to_string()]
    ));
}

#[test]
fn readdir_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("readdir-deny");
    std::fs::write(temp.join("a.txt"), b"a").expect("seed a");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-readdir", &temp.join("").display().to_string()]
    ));
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
