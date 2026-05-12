//! c01 — `rfl init` from a cwd without a pre-existing lock exits
//! non-zero with `NotYetImplemented`. This assertion is **amended
//! away** in c02 once the writer lands (two-stage ladder per m0 §4.3).

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_init_without_lock_not_yet_implemented() {
    let project = tempfile::tempdir().unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--project-root")
        .arg(project.path())
        .output()
        .expect("spawn rfl init");

    assert!(
        !out.status.success(),
        "rfl init should fail without a pre-existing lock in c01; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("NotYetImplemented"),
        "stderr missing NotYetImplemented marker: {stderr}"
    );
}
