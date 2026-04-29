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
        common::sandbox_builder().read_path(allowed.clone()),
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
        common::sandbox_builder().read_path(allowed),
        &["can-read", &sibling.display().to_string()]
    ));
}

// ── directory read scoping ────────────────────────────────────

#[test]
fn read_directory_grants_access_to_children() {
    let temp = TestDir::new("read-dir");
    let child_file = temp.join("nested.txt");
    std::fs::write(&child_file, b"nested content").expect("write nested file");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_dir(dir),
        &["can-read", &child_file.display().to_string()]
    ));
}

// ── directory write scoping ──────────────────────────────────

#[test]
fn write_directory_grants_write_to_children() {
    let temp = TestDir::new("write-dir");
    let child_file = temp.join("writable.txt");
    std::fs::write(&child_file, b"seed").expect("seed child file");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().write_dir(dir),
        &["can-write", &child_file.display().to_string()]
    ));
}

// ── read-only enforcement ────────────────────────────────────

#[test]
fn read_path_blocks_writes() {
    let temp = TestDir::new("ro-enforce");
    let target = temp.join("readonly.txt");
    std::fs::write(&target, b"seed").expect("seed target");

    assert!(
        run_probe(
            common::sandbox_builder().read_path(target.clone()),
            &["can-read", &target.display().to_string()]
        ),
        "read should succeed on read_path"
    );
    assert!(
        !run_probe(
            common::sandbox_builder().read_path(target.clone()),
            &["can-write", &target.display().to_string()]
        ),
        "write should fail on read_path"
    );
}

// ── readdir scoping ──────────────────────────────────────────

#[test]
fn read_directory_grants_readdir() {
    let temp = TestDir::new("readdir-ro");
    std::fs::write(temp.join("a.txt"), b"a").expect("seed a");
    std::fs::write(temp.join("b.txt"), b"b").expect("seed b");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().read_dir(dir.clone()),
        &["can-readdir", &dir.display().to_string()]
    ));
}

#[test]
fn write_directory_grants_readdir() {
    let temp = TestDir::new("readdir-rw");
    std::fs::write(temp.join("a.txt"), b"a").expect("seed a");

    let dir = temp.join("");
    assert!(run_probe(
        common::sandbox_builder().write_dir(dir.clone()),
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
        common::sandbox_builder().read_dir(dir),
        &["can-readdir", &subdir.display().to_string()]
    ));
}

// ── mkdir scoping ────────────────────────────────────────────

#[test]
fn write_directory_grants_mkdir() {
    let temp = TestDir::new("mkdir-rw");
    let new_dir = temp.join("created");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &["can-mkdir", &new_dir.display().to_string()]
    ));
    assert!(
        new_dir.is_dir(),
        "probe reported success but directory was not created on disk"
    );
}

#[test]
fn read_directory_does_not_grant_mkdir() {
    let temp = TestDir::new("mkdir-ro");
    let new_dir = temp.join("blocked");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &["can-mkdir", &new_dir.display().to_string()]
    ));
}

// ── truncate scoping ─────────────────────────────────────────

#[test]
fn write_directory_grants_truncate() {
    let temp = TestDir::new("truncate-rw");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed contents").expect("seed file");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &["can-truncate", &target.display().to_string()]
    ));
}

#[test]
fn read_directory_does_not_grant_truncate() {
    let temp = TestDir::new("truncate-ro");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &["can-truncate", &target.display().to_string()]
    ));
}

#[test]
fn truncate_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("truncate-deny");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-truncate", &target.display().to_string()]
    ));
}

// ── utime scoping ────────────────────────────────────────────

#[test]
fn write_directory_grants_utime() {
    let temp = TestDir::new("utime-rw");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &["can-utime", &target.display().to_string()]
    ));
}

#[test]
fn read_directory_does_not_grant_utime() {
    let temp = TestDir::new("utime-ro");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &["can-utime", &target.display().to_string()]
    ));
}

#[test]
fn utime_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("utime-deny");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-utime", &target.display().to_string()]
    ));
}

// ── rmdir scoping ────────────────────────────────────────────

#[test]
fn write_directory_grants_rmdir() {
    let temp = TestDir::new("rmdir-rw");
    let victim = temp.join("victim");
    std::fs::create_dir(&victim).expect("seed dir");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &["can-rmdir", &victim.display().to_string()]
    ));
    assert!(!victim.exists(), "dir should have been removed");
}

#[test]
fn read_directory_does_not_grant_rmdir() {
    let temp = TestDir::new("rmdir-ro");
    let victim = temp.join("victim");
    std::fs::create_dir(&victim).expect("seed dir");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &["can-rmdir", &victim.display().to_string()]
    ));
    assert!(victim.exists(), "dir should NOT have been removed");
}

#[test]
fn rmdir_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("rmdir-deny");
    let victim = temp.join("victim");
    std::fs::create_dir(&victim).expect("seed dir");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-rmdir", &victim.display().to_string()]
    ));
}

// ── rename scoping ───────────────────────────────────────────

#[test]
fn write_directory_grants_rename() {
    let temp = TestDir::new("rename-rw");
    let from = temp.join("from.txt");
    let to = temp.join("to.txt");
    std::fs::write(&from, b"seed").expect("seed file");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &[
            "can-rename",
            &from.display().to_string(),
            &to.display().to_string()
        ]
    ));
    assert!(
        to.exists() && !from.exists(),
        "rename should have moved the file"
    );
}

#[test]
fn read_directory_does_not_grant_rename() {
    let temp = TestDir::new("rename-ro");
    let from = temp.join("from.txt");
    let to = temp.join("to.txt");
    std::fs::write(&from, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &[
            "can-rename",
            &from.display().to_string(),
            &to.display().to_string()
        ]
    ));
}

#[test]
fn rename_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("rename-deny");
    let from = temp.join("from.txt");
    let to = temp.join("to.txt");
    std::fs::write(&from, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder(),
        &[
            "can-rename",
            &from.display().to_string(),
            &to.display().to_string()
        ]
    ));
}

// ── unlink scoping ───────────────────────────────────────────

#[test]
fn write_directory_grants_unlink() {
    let temp = TestDir::new("unlink-rw");
    let target = temp.join("victim.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &["can-unlink", &target.display().to_string()]
    ));
    assert!(!target.exists(), "file should have been removed");
}

#[test]
fn read_directory_does_not_grant_unlink() {
    let temp = TestDir::new("unlink-ro");
    let target = temp.join("victim.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &["can-unlink", &target.display().to_string()]
    ));
    assert!(target.exists(), "file should NOT have been removed");
}

#[test]
fn unlink_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("unlink-deny");
    let target = temp.join("victim.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-unlink", &target.display().to_string()]
    ));
}

// ── chmod scoping ────────────────────────────────────────────

#[test]
fn write_directory_grants_chmod() {
    let temp = TestDir::new("chmod-rw");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(run_probe(
        common::sandbox_builder().write_dir(temp.join("")),
        &["can-chmod", &target.display().to_string(), "0o644"]
    ));
}

#[test]
fn read_directory_does_not_grant_chmod() {
    let temp = TestDir::new("chmod-ro");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder().read_dir(temp.join("")),
        &["can-chmod", &target.display().to_string(), "0o644"]
    ));
}

#[test]
fn chmod_is_denied_without_explicit_allowlist() {
    let temp = TestDir::new("chmod-deny");
    let target = temp.join("file.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-chmod", &target.display().to_string(), "0o644"]
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
        common::sandbox_builder().read_path(allowed.clone()),
        &["can-stat", &allowed.display().to_string()]
    ));
    assert!(!run_probe(
        common::sandbox_builder().read_path(allowed),
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
        common::sandbox_builder().write_path(allowed.clone()),
        &["can-write", &allowed.display().to_string()]
    ));
    assert!(!run_probe(
        common::sandbox_builder().write_path(allowed),
        &["can-write", &denied.display().to_string()]
    ));
}

#[test]
fn write_path_can_truncate_existing() {
    let temp = TestDir::new("write-path-truncate");
    let target = temp.join("existing.txt");
    std::fs::write(&target, b"seed contents").expect("seed file");

    assert!(run_probe(
        common::sandbox_builder().write_path(target.clone()),
        &["can-truncate", &target.display().to_string()]
    ));
    let contents = std::fs::read(&target).expect("read truncated file");
    assert!(
        contents.is_empty(),
        "probe reported success but file was not truncated on disk: {contents:?}"
    );
}

#[test]
fn write_path_does_not_grant_unlink() {
    let temp = TestDir::new("write-path-no-unlink");
    let target = temp.join("victim.txt");
    std::fs::write(&target, b"seed").expect("seed file");

    assert!(!run_probe(
        common::sandbox_builder().write_path(target.clone()),
        &["can-unlink", &target.display().to_string()]
    ));
    assert!(target.exists(), "file should NOT have been removed");
}

#[test]
fn write_path_does_not_grant_create() {
    let temp = TestDir::new("write-path-no-create");
    let target = temp.join("new.log");

    assert!(!run_probe(
        common::sandbox_builder().write_path(target.clone()),
        &["can-create-file", &target.display().to_string()]
    ));
    assert!(!target.exists(), "file should NOT have been created");
}

// ── exec scoping ─────────────────────────────────────────────

fn pick_system_true() -> &'static str {
    ["/usr/bin/true", "/bin/true"]
        .iter()
        .copied()
        .find(|p| std::path::Path::new(p).exists())
        .expect("expected /usr/bin/true or /bin/true on host")
}

#[test]
fn exec_path_allows_executing_the_named_binary() {
    let target = pick_system_true();
    assert!(run_probe(
        common::sandbox_builder().exec_path(target),
        &["can-exec", target]
    ));
}

#[test]
fn exec_path_grants_read_too() {
    let target = pick_system_true();
    assert!(
        run_probe(
            common::sandbox_builder().exec_path(target),
            &["can-read", target]
        ),
        "exec_path should imply read access on the same path"
    );
}

#[test]
fn exec_dir_allows_executing_anything_in_the_tree() {
    let target = pick_system_true();
    let parent = std::path::Path::new(target)
        .parent()
        .expect("system binary has a parent dir")
        .to_path_buf();

    assert!(run_probe(
        common::sandbox_builder().exec_dir(parent),
        &["can-exec", target]
    ));
}

#[test]
fn read_dir_does_not_allow_exec() {
    let target = pick_system_true();
    let parent = std::path::Path::new(target)
        .parent()
        .expect("system binary has a parent dir")
        .to_path_buf();

    assert!(
        !run_probe(
            common::sandbox_builder().read_dir(parent),
            &["can-exec", target]
        ),
        "read_dir must not grant exec — that's the whole point of a separate exec capability"
    );
}

// ── interactive tty scoping ──────────────────────────────────

#[test]
fn interactive_tty_does_not_grant_other_devices() {
    assert!(!run_probe(
        common::sandbox_builder().allow_interactive_tty(true),
        &["can-read", "/dev/console"]
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

// ── symlink / hardlink escape regressions ────────────────────
//
// Adversarial cases: a symlink that lives inside the allowlist
// must not become a backdoor to content outside it, and a hardlink
// that lives inside a writable allowlist exposes inode-vs-path
// semantics worth pinning in a test.

#[test]
fn symlink_inside_read_dir_does_not_grant_outside_read() {
    let temp = TestDir::new("symlink-ro-escape");
    let secret_dir = temp.join("secret_dir");
    let inside_dir = temp.join("inside_dir");
    std::fs::create_dir(&secret_dir).expect("mkdir secret_dir");
    std::fs::create_dir(&inside_dir).expect("mkdir inside_dir");

    let secret = secret_dir.join("secret.txt");
    std::fs::write(&secret, b"secret").expect("seed secret");

    let link = inside_dir.join("link");
    std::os::unix::fs::symlink(&secret, &link).expect("create symlink");

    // Allow ONLY inside_dir. secret_dir is outside the allowlist, so
    // a symlink resolving into it must not bypass enforcement.
    assert!(
        !run_probe(
            common::sandbox_builder().read_dir(inside_dir),
            &["can-read", &link.display().to_string()]
        ),
        "reading through a symlink that points outside the allowlist must be denied"
    );
}

#[test]
fn symlink_inside_write_dir_cannot_be_used_to_write_outside() {
    let temp = TestDir::new("symlink-rw-escape");
    let outside_dir = temp.join("outside_dir");
    let inside_dir = temp.join("inside_dir");
    std::fs::create_dir(&outside_dir).expect("mkdir outside_dir");
    std::fs::create_dir(&inside_dir).expect("mkdir inside_dir");

    let target = outside_dir.join("target.txt");
    std::fs::write(&target, b"orig").expect("seed target");

    let link = inside_dir.join("link");
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");

    assert!(
        !run_probe(
            common::sandbox_builder().write_dir(inside_dir),
            &["can-write", &link.display().to_string()]
        ),
        "writing through a symlink that points outside the allowlist must be denied"
    );

    let after = std::fs::read(&target).expect("read target after attempt");
    assert_eq!(
        after, b"orig",
        "outside file must be unchanged after symlink-write attempt"
    );
}

#[test]
fn hardlink_inside_write_dir_to_outside_file_documents_behavior() {
    // Hardlinks share an inode, but path-based sandboxes (syd +
    // landlock on Linux, sandbox-exec on macOS) police *paths*, not
    // inodes. An inside-allowlist path that hardlinks to outside
    // content is therefore reachable: the policy names the path the
    // child uses, and that path lives in the allowed dir. This is
    // intentional — callers must not place hardlinks to sensitive
    // content into a writable allowed dir.
    let temp = TestDir::new("hardlink-rw");
    let outside_dir = temp.join("outside_dir");
    let inside_dir = temp.join("inside_dir");
    std::fs::create_dir(&outside_dir).expect("mkdir outside_dir");
    std::fs::create_dir(&inside_dir).expect("mkdir inside_dir");

    let target = outside_dir.join("target.txt");
    std::fs::write(&target, b"orig").expect("seed target");

    let link = inside_dir.join("link");
    std::fs::hard_link(&target, &link).expect("create hardlink");

    // Reading via the inside path should succeed: the path is in the
    // allowlist regardless of which inode it points at.
    assert!(
        run_probe(
            common::sandbox_builder().write_dir(inside_dir.clone()),
            &["can-read", &link.display().to_string()]
        ),
        "reading the hardlink via its inside-allowlist path is expected to succeed \
         under path-based enforcement; if this starts failing the policy semantics \
         changed"
    );

    // Reading via the outside path must still be denied — the outside
    // dir is not in the allowlist.
    assert!(
        !run_probe(
            common::sandbox_builder().write_dir(inside_dir),
            &["can-read", &target.display().to_string()]
        ),
        "reading via the outside path must remain denied even though an inside \
         hardlink exists"
    );
}
