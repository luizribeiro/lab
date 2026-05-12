//! c12 (scope §D2 + §D3) — `--since` parses `<n><m|h|d>` and applies
//! `WHERE at >= ?` against the audit row's UTC RFC3339 timestamp.
//! Rows older than the threshold are excluded; rows newer are kept.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use serde_json::json;

#[test]
fn rfl_audit_filters_by_since() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    writer
        .record(AuditKind::ConfirmRequest, None, &json!({"tag": "fresh"}))
        .unwrap();

    let db_path = project
        .path()
        .join(".rafaello")
        .join("state")
        .join("session.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let two_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    let opt_req: Option<String> = None;
    conn.execute(
        "INSERT INTO audit_events (at, kind, request_id, payload) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            two_hours_ago,
            "confirm_request",
            opt_req,
            "{\"tag\":\"stale\"}"
        ],
    )
    .unwrap();
    drop(conn);

    let rfl = workspace_bin("rfl");

    let out_1h = Command::new(&rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--since", "1h"])
        .output()
        .unwrap();
    assert!(
        out_1h.status.success(),
        "rfl audit --since 1h failed: stderr={}",
        String::from_utf8_lossy(&out_1h.stderr)
    );
    let stdout_1h = String::from_utf8_lossy(&out_1h.stdout);
    assert!(
        stdout_1h.contains("fresh"),
        "expected --since 1h to include fresh row: {stdout_1h}"
    );
    assert!(
        !stdout_1h.contains("stale"),
        "expected --since 1h to exclude 2h-old row: {stdout_1h}"
    );

    let out_30m = Command::new(&rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--since", "30m"])
        .output()
        .unwrap();
    assert!(out_30m.status.success());
    let stdout_30m = String::from_utf8_lossy(&out_30m.stdout);
    assert!(
        stdout_30m.contains("fresh"),
        "expected --since 30m to include fresh row: {stdout_30m}"
    );
    assert!(
        !stdout_30m.contains("stale"),
        "expected --since 30m to exclude 2h-old row: {stdout_30m}"
    );
}

#[test]
fn rfl_audit_since_invalid_spec_errors() {
    let project = tempfile::tempdir().unwrap();
    let _writer = AuditWriter::open_for_install(project.path()).unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--since", "garbage"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected --since garbage to exit non-zero, stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid --since spec"),
        "expected diagnostic about invalid spec, got: {stderr}"
    );
}
