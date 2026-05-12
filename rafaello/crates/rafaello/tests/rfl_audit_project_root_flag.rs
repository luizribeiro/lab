//! c11 (scope §D3 round-3 B-2) — `rfl audit --project-root <tmpdir>`
//! invoked from a different cwd reads the audit DB under `<tmpdir>`,
//! and produces the same output as running with cwd = `<tmpdir>`.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use serde_json::json;

#[test]
fn rfl_audit_project_root_flag() {
    let project = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");
    writer
        .record(AuditKind::ConfirmRequest, None, &json!({"row": 1}))
        .expect("record");
    writer
        .record(AuditKind::ConfirmAllowed, None, &json!({"row": 2}))
        .expect("record");

    let rfl = workspace_bin("rfl");

    let from_elsewhere = Command::new(&rfl)
        .current_dir(elsewhere.path())
        .args(["audit", "--project-root"])
        .arg(project.path())
        .output()
        .expect("spawn rfl audit from elsewhere");
    assert!(
        from_elsewhere.status.success(),
        "rfl audit (from elsewhere) failed: stderr={}",
        String::from_utf8_lossy(&from_elsewhere.stderr)
    );

    let from_project = Command::new(&rfl)
        .current_dir(project.path())
        .arg("audit")
        .output()
        .expect("spawn rfl audit from project");
    assert!(
        from_project.status.success(),
        "rfl audit (from project) failed: stderr={}",
        String::from_utf8_lossy(&from_project.stderr)
    );

    assert_eq!(
        String::from_utf8_lossy(&from_elsewhere.stdout),
        String::from_utf8_lossy(&from_project.stdout),
        "stdout differs between --project-root invocation and cwd invocation"
    );

    assert!(
        !elsewhere.path().join(".rafaello").exists(),
        ".rafaello/ leaked into invoking cwd"
    );
}
