//! c12 (scope §D2 + §D3) — `rfl audit --kind confirm_request --kind
//! confirm_allowed` returns the union of the two kinds and excludes
//! every other `AuditKind`.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use serde_json::json;

#[test]
fn rfl_audit_filters_by_kind() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    writer
        .record(AuditKind::ConfirmRequest, None, &json!({"k": "a"}))
        .unwrap();
    writer
        .record(AuditKind::ConfirmAllowed, None, &json!({"k": "b"}))
        .unwrap();
    writer
        .record(AuditKind::ConfirmDenied, None, &json!({"k": "c"}))
        .unwrap();
    writer
        .record(AuditKind::GrantAdded, None, &json!({"k": "d"}))
        .unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--kind", "confirm_request", "--kind", "confirm_allowed"])
        .output()
        .expect("spawn rfl audit");
    assert!(
        out.status.success(),
        "rfl audit failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "expected 2 union-of-kinds rows, got {}: {stdout}",
        lines.len()
    );
    assert!(
        stdout.contains("confirm_request"),
        "missing confirm_request row: {stdout}"
    );
    assert!(
        stdout.contains("confirm_allowed"),
        "missing confirm_allowed row: {stdout}"
    );
    assert!(
        !stdout.contains("confirm_denied"),
        "confirm_denied leaked through filter: {stdout}"
    );
    assert!(
        !stdout.contains("grant_added"),
        "grant_added leaked through filter: {stdout}"
    );
}
