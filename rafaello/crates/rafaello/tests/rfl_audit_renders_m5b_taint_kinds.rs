//! c13 (scope §D3) — default render distinguishes the m5b taint
//! `AuditKind` variants in the `<kind>` column:
//!   - `ConfirmRequestTaintAttached` (m5b row 58)
//!   - `PluginPublishRejectedTaintSuperset` (m5b row 55)
//!   - `ToolRequestTaintUnionedFromInReplyTo` (m5b row 57)

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use rafaello_core::bus::JsonRpcId;
use serde_json::json;

#[test]
fn rfl_audit_renders_m5b_taint_kinds() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    let rid_58 = JsonRpcId::from("req-58");
    let rid_57 = JsonRpcId::from("req-57");
    writer
        .record(
            AuditKind::ConfirmRequestTaintAttached,
            Some(&rid_58),
            &json!({"row": 58}),
        )
        .unwrap();
    writer
        .record(
            AuditKind::PluginPublishRejectedTaintSuperset,
            None,
            &json!({"row": 55}),
        )
        .unwrap();
    writer
        .record(
            AuditKind::ToolRequestTaintUnionedFromInReplyTo,
            Some(&rid_57),
            &json!({"row": 57}),
        )
        .unwrap();

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
        3,
        "expected 3 rendered rows, got {}: {stdout}",
        lines.len()
    );

    let confirm_line = lines
        .iter()
        .find(|l| l.contains("confirm_request_taint_attached"))
        .unwrap_or_else(|| panic!("missing confirm_request_taint_attached row: {stdout}"));
    let publish_line = lines
        .iter()
        .find(|l| l.contains("plugin_publish_rejected_taint_superset"))
        .unwrap_or_else(|| panic!("missing plugin_publish_rejected_taint_superset row: {stdout}"));
    let tool_line = lines
        .iter()
        .find(|l| l.contains("tool_request_taint_unioned_from_in_reply_to"))
        .unwrap_or_else(|| {
            panic!("missing tool_request_taint_unioned_from_in_reply_to row: {stdout}")
        });

    assert_ne!(confirm_line, publish_line);
    assert_ne!(confirm_line, tool_line);
    assert_ne!(publish_line, tool_line);

    assert!(
        !confirm_line.contains("plugin_publish_rejected_taint_superset"),
        "kind column conflated taint kinds: {confirm_line}"
    );
    assert!(
        !confirm_line.contains("tool_request_taint_unioned_from_in_reply_to"),
        "kind column conflated taint kinds: {confirm_line}"
    );
}
