//! c13 (scope §D3) — `--kind confirm_request --since 1h --request-id <id>`
//! composes with AND semantics: the built SQL must emit a single combined
//! `WHERE` clause that joins all three predicates with `AND`.

use rafaello::audit_cli::build_query;
use rusqlite::types::Value;

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use rafaello_core::bus::JsonRpcId;
use serde_json::json;

#[test]
fn build_query_combines_all_filters_with_single_where_and_and_semantics() {
    let since = chrono::Utc::now() - chrono::Duration::hours(1);
    let q = build_query(
        &["confirm_request".to_string()],
        Some("req-77"),
        Some(&since),
    );

    let where_count = q.sql.matches(" WHERE ").count();
    assert_eq!(
        where_count, 1,
        "expected exactly one WHERE clause, got {where_count}: {}",
        q.sql
    );

    let and_count = q.sql.matches(" AND ").count();
    assert_eq!(
        and_count, 2,
        "expected two AND joiners across three predicates, got {and_count}: {}",
        q.sql
    );

    assert!(
        q.sql.contains("kind IN (?)"),
        "missing kind predicate: {}",
        q.sql
    );
    assert!(
        q.sql.contains("request_id = ?"),
        "missing request_id predicate: {}",
        q.sql
    );
    assert!(
        q.sql.contains("at >= ?"),
        "missing since predicate: {}",
        q.sql
    );

    assert_eq!(
        q.params.len(),
        3,
        "expected 3 bound params, got {}",
        q.params.len()
    );
    match &q.params[0] {
        Value::Text(t) => assert_eq!(t, "confirm_request"),
        other => panic!("expected text kind param, got {other:?}"),
    }
    match &q.params[1] {
        Value::Text(t) => assert_eq!(t, "req-77"),
        other => panic!("expected text request_id param, got {other:?}"),
    }
    match &q.params[2] {
        Value::Text(_) => {}
        other => panic!("expected text since param, got {other:?}"),
    }
}

#[test]
fn rfl_audit_filters_combine_end_to_end() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    let rid_77 = JsonRpcId::from("req-77");
    let rid_other = JsonRpcId::from("req-other");
    writer
        .record(
            AuditKind::ConfirmRequest,
            Some(&rid_77),
            &json!({"want": true}),
        )
        .unwrap();
    writer
        .record(
            AuditKind::ConfirmRequest,
            Some(&rid_other),
            &json!({"want": false}),
        )
        .unwrap();
    writer
        .record(
            AuditKind::ConfirmAllowed,
            Some(&rid_77),
            &json!({"want": false}),
        )
        .unwrap();
    writer
        .record(AuditKind::ConfirmDenied, None, &json!({"want": false}))
        .unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args([
            "--kind",
            "confirm_request",
            "--since",
            "1h",
            "--request-id",
            "req-77",
        ])
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
        1,
        "expected exactly one row matching all three filters, got {}: {stdout}",
        lines.len()
    );
    let line = lines[0];
    assert!(line.contains("confirm_request"), "missing kind: {line}");
    assert!(line.contains("req-77"), "missing request_id: {line}");
    assert!(
        !stdout.contains("req-other"),
        "leaked req-other row: {stdout}"
    );
    assert!(
        !stdout.contains("confirm_allowed"),
        "leaked confirm_allowed row: {stdout}"
    );
    assert!(
        !stdout.contains("confirm_denied"),
        "leaked confirm_denied row: {stdout}"
    );
}
