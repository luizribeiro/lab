//! c01 — `rfl init` with a pre-existing `rafaello.lock` is idempotent:
//! exit 0, lock bytes unchanged, stderr contains `lock already present`
//! (scope §A1 hard requirement #1, idempotency invariant).

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_init_with_existing_lock_idempotent() {
    let project = tempfile::tempdir().unwrap();
    let lock_path = project.path().join("rafaello.lock");
    let bytes: &[u8] = b"arbitrary bytes that are not a valid lock\n";
    fs::write(&lock_path, bytes).unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--project-root")
        .arg(project.path())
        .output()
        .expect("spawn rfl init");

    assert!(
        out.status.success(),
        "rfl init should exit 0 when lock already exists; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = fs::read(&lock_path).unwrap();
    assert_eq!(after, bytes, "lock bytes were modified");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("lock already present"),
        "stderr missing idempotency notice: {stderr}"
    );
}
