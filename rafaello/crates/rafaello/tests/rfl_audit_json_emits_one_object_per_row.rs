//! c12 (scope §D2 + §D3) — `--json` emits one JSON object per row
//! with keys `seq, at, kind, request_id, payload`. The payload key
//! holds the parsed JSON value (an object), not a stringified copy.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use serde_json::{json, Value};

#[test]
fn rfl_audit_json_emits_one_object_per_row() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    let payloads = [
        json!({"call_id": "alpha", "tool": "read_file"}),
        json!({"call_id": "beta", "extra": {"nested": true}}),
        json!({"call_id": "gamma"}),
    ];
    for payload in &payloads {
        writer
            .record(AuditKind::ConfirmRequest, None, payload)
            .unwrap();
    }

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--json"])
        .output()
        .expect("spawn rfl audit --json");
    assert!(
        out.status.success(),
        "rfl audit --json failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        payloads.len(),
        "expected one JSON object per row, got: {stdout}"
    );

    for (idx, line) in lines.iter().enumerate() {
        let value: Value = serde_json::from_str(line)
            .unwrap_or_else(|err| panic!("line {idx} not JSON ({err}): {line}"));
        assert!(
            value.get("seq").and_then(Value::as_i64).is_some(),
            "seq missing/non-int: {line}"
        );
        assert!(
            value.get("at").and_then(Value::as_str).is_some(),
            "at missing: {line}"
        );
        assert_eq!(
            value.get("kind").and_then(Value::as_str),
            Some("confirm_request"),
            "kind mismatch in {line}"
        );
        assert!(
            value.get("request_id").is_some(),
            "request_id key missing in {line}"
        );
        let payload = value.get("payload").expect("payload key missing");
        assert!(
            payload.is_object(),
            "payload must be parsed JSON object (not stringified): got {payload:?}"
        );
        assert!(
            payload.get("call_id").and_then(Value::as_str).is_some(),
            "payload.call_id missing in {line}"
        );
    }
}
