//! c11 — `rfl audit --project-root <tmpdir>` lists every row inserted
//! by `AuditWriter::open_for_install` + `record(...)` in `seq ASC`
//! order, one row per line in the documented column layout
//! `<seq>  <at>  <kind>  [<request_id>|-]  <payload-summary>`
//! (scope §D1 + §D3).

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use serde_json::json;

#[test]
fn rfl_audit_lists_all_rows_from_live_schema() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    let payloads = [
        json!({"call_id": "alpha"}),
        json!({"call_id": "beta", "tool": "read_file"}),
        json!({"call_id": "gamma"}),
    ];
    for payload in &payloads {
        writer
            .record(AuditKind::ConfirmRequest, None, payload)
            .expect("record audit row");
    }

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
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
        payloads.len(),
        "expected {} rows, got {}: {stdout}",
        payloads.len(),
        lines.len()
    );

    let first = lines[0];
    let mut cols = first.split("  ");
    let seq = cols.next().expect("seq column");
    assert!(
        seq.parse::<i64>().is_ok(),
        "first column not numeric seq: {first}"
    );
    let at = cols.next().expect("at column");
    assert!(
        at.contains('T') && (at.contains('+') || at.contains('Z') || at.contains('-')),
        "second column not an rfc3339 timestamp: {first}"
    );
    let kind = cols.next().expect("kind column");
    assert_eq!(kind, "confirm_request", "third column not the audit kind");
    let request_id = cols.next().expect("request_id column");
    assert_eq!(
        request_id, "[-]",
        "fourth column not the request_id slot (got {request_id})"
    );
    let summary = cols.next().expect("payload-summary column");
    assert!(
        summary.contains("alpha"),
        "fifth column does not contain payload contents: {first}"
    );
}
