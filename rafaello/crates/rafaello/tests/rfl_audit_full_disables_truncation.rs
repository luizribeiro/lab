//! c12 (scope §D2 + §D3) — `--full` disables the default
//! 80-character payload-summary truncation; the default render path
//! truncates payloads larger than the summary window.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::audit::{AuditKind, AuditWriter};
use serde_json::json;

#[test]
fn rfl_audit_full_disables_truncation() {
    let project = tempfile::tempdir().unwrap();
    let writer = AuditWriter::open_for_install(project.path()).expect("open_for_install");

    let big_value: String = "x".repeat(1024);
    let payload = json!({
        "aaa_head": "OPEN_MARKER",
        "mmm_filler": big_value,
        "zzz_tail": "TAIL_MARKER",
    });
    writer
        .record(AuditKind::ConfirmRequest, None, &payload)
        .unwrap();

    let rfl = workspace_bin("rfl");

    let default_out = Command::new(&rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .output()
        .unwrap();
    assert!(
        default_out.status.success(),
        "rfl audit failed: stderr={}",
        String::from_utf8_lossy(&default_out.stderr)
    );
    let default_stdout = String::from_utf8_lossy(&default_out.stdout);
    assert!(
        default_stdout.contains("OPEN_MARKER"),
        "default render should show payload head: {default_stdout}"
    );
    assert!(
        !default_stdout.contains("TAIL_MARKER"),
        "default render should truncate 1KB payload before tail: {default_stdout}"
    );

    let full_out = Command::new(&rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .args(["--full"])
        .output()
        .unwrap();
    assert!(full_out.status.success());
    let full_stdout = String::from_utf8_lossy(&full_out.stdout);
    assert!(
        full_stdout.contains("OPEN_MARKER"),
        "--full output missing head: {full_stdout}"
    );
    assert!(
        full_stdout.contains("TAIL_MARKER"),
        "--full output missing tail (should not truncate): head_len={} stdout_len={}",
        full_stdout.find("OPEN_MARKER").unwrap_or(0),
        full_stdout.len()
    );
    assert!(
        full_stdout.contains(&"x".repeat(1024)),
        "--full output missing complete 1KB filler"
    );
}
