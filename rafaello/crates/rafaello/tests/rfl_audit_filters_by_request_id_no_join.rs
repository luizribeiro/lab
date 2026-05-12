//! c12 (scope §D2 + §D3) — `--request-id` filters by audit row
//! `request_id` only; the executed SQL must reference
//! `FROM audit_events` and must NOT touch the `entries` table.
//!
//! The live `entries` schema has no `call_id` column (scope §"Out of
//! scope" item 10), so this test fences future maintainers off from
//! re-introducing a join that would not compile against the real
//! session-store schema.

use rafaello::audit_cli::build_query;
use rusqlite::types::Value;

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::AuditWriter;

#[test]
fn build_query_request_id_only_targets_audit_events_and_avoids_entries() {
    let q = build_query(&[], Some("req-42"), None);
    assert!(
        q.sql.contains("FROM audit_events"),
        "sql does not target audit_events: {}",
        q.sql
    );
    assert!(
        !q.sql.to_lowercase().contains("entries"),
        "sql leaked the entries table: {}",
        q.sql
    );
    assert!(
        q.sql.contains("request_id = ?"),
        "sql missing request_id = ? predicate: {}",
        q.sql
    );
    assert_eq!(q.params.len(), 1, "expected single bound param");
    match &q.params[0] {
        Value::Text(t) => assert_eq!(t, "req-42"),
        other => panic!("expected text param, got {other:?}"),
    }
}

#[test]
fn build_query_with_all_filters_still_avoids_entries() {
    let since = chrono::Utc::now() - chrono::Duration::hours(1);
    let q = build_query(
        &["confirm_request".to_string()],
        Some("req-99"),
        Some(&since),
    );
    assert!(q.sql.contains("FROM audit_events"));
    assert!(
        !q.sql.to_lowercase().contains("entries"),
        "sql leaked the entries table: {}",
        q.sql
    );
    assert!(q.sql.contains("kind IN (?)"));
    assert!(q.sql.contains("request_id = ?"));
    assert!(q.sql.contains("at >= ?"));
}

#[test]
fn rfl_audit_request_id_end_to_end() {
    let project = tempfile::tempdir().unwrap();
    let _writer = AuditWriter::open_for_install(project.path()).unwrap();

    let db_path = project
        .path()
        .join(".rafaello")
        .join("state")
        .join("session.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let some_req: Option<&str> = Some("req-42");
    let no_req: Option<&str> = None;
    conn.execute(
        "INSERT INTO audit_events (at, kind, request_id, payload) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![now, "confirm_request", some_req, "{\"x\":1}"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO audit_events (at, kind, request_id, payload) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![now, "confirm_allowed", no_req, "{\"x\":2}"],
    )
    .unwrap();
    drop(conn);

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--request-id", "req-42"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rfl audit failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "expected one matching row, got {stdout}");
    assert!(
        stdout.contains("req-42"),
        "row missing request_id: {stdout}"
    );
    assert!(
        stdout.contains("confirm_request"),
        "row missing kind: {stdout}"
    );
}
