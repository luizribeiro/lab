//! c40 — Scope §"Demo bar" §Bonus: a tool with `sinks = []` +
//! `always_confirm = true` still fires the confirmation prompt.
//!
//! Same wiring as c39's headline but the mailcat tool_meta is
//! overridden in the materialised lock so `sinks = []` and
//! `always_confirm = true`. The gate computes `gate_required =
//! !sinks.is_empty() || always_confirm` (rafaello-core::gate), so it
//! must hold the request even without any declared sink. We answer
//! `deny` so mailcat stays silent and the audit log records
//! `confirm_request` + `confirm_denied`.

mod common;

use std::process::Command;

use common::m5a_demo_kit::{
    audit_kinds, install_m5a_demo_lock, mailcat_log_path, stub_send_mail_then_text,
    MailcatToolMetaOverrides, OpenAiStub,
};
use common::workspace_bin_path::workspace_bin;
use rusqlite::Connection;
use serial_test::serial;

#[test]
#[serial(rfl_chat)]
fn rfl_chat_always_confirm_true_holds_non_sink_tool() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-openai");
    let _ = workspace_bin("rfl-mailcat");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();

    let stub = OpenAiStub::start(stub_send_mail_then_text(
        "Understood; I will not send the email.",
    ));
    install_m5a_demo_lock(
        project_root,
        &stub.endpoint(),
        MailcatToolMetaOverrides {
            sinks: Vec::new(),
            always_confirm: true,
            grant_match_path: None,
        },
    );

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "please email alice")
        .env("RFL_TUI_TEST_CONFIRM_ANSWER", "deny")
        .env("RFL_TUI_MAX_LIFETIME", "10")
        .env("LITELLM_API_KEY", "sk-test-demo-bar")
        .output()
        .expect("spawn rfl chat");

    drop(stub);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected zero exit; stderr={stderr}"
    );

    let state_dir = project_root.join(".rafaello").join("state");
    let conn = Connection::open(state_dir.join("session.sqlite")).expect("open audit sqlite");
    let kinds = audit_kinds(&conn);
    assert!(
        kinds.contains(&"confirm_request".to_string()),
        "audit missing confirm_request (gate did not hold the always_confirm tool); got {kinds:?}"
    );
    assert!(
        kinds.contains(&"confirm_denied".to_string()),
        "audit missing confirm_denied; got {kinds:?}"
    );
    assert!(
        !kinds.contains(&"gate_passthrough".to_string()),
        "gate must not pass through when always_confirm=true; got {kinds:?}"
    );

    let log_path = mailcat_log_path(project_root);
    let mailcat_empty = !log_path.exists()
        || std::fs::metadata(&log_path)
            .map(|m| m.len() == 0)
            .unwrap_or(true);
    assert!(
        mailcat_empty,
        "mailcat.log must stay empty on deny; path={log_path:?}"
    );
}
