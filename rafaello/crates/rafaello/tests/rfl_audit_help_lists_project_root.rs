//! c11 — `rfl audit --help` exits 0 and the usage prints
//! `--project-root <PATH>` (scope §D1).

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_audit_help_lists_project_root() {
    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--help"])
        .output()
        .expect("spawn rfl audit --help");
    assert!(
        out.status.success(),
        "rfl audit --help failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--project-root"),
        "help missing --project-root: {stdout}"
    );
    assert!(
        stdout.contains("<PATH>") || stdout.contains("<PROJECT_ROOT>"),
        "help missing PATH placeholder: {stdout}"
    );
}
