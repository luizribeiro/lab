//! c40 — Scope §"Demo bar" §Negative 1: confirmation timeout denies.
//!
//! Same setup as c39's `rfl_chat_demo_bar_send_mail.rs` headline, but
//! `RFL_TUI_TEST_CONFIRM_ANSWER=timeout` — the TUI receives the
//! `confirm_request` and deliberately publishes no answer. After the
//! gate's 60s deadline expires the gate synthesises a deny
//! `tool_result` (§CG4a; the wire-level shape — `taint = [{source:
//! "system", detail: "confirm_timeout"}]`, `error = "confirm_timeout"`,
//! `in_reply_to = [held_id]` — is already pinned by
//! `gate_synthesises_deny_tool_result_with_pinned_shape` in
//! rafaello-core; this test verifies the integration-level
//! consequences: entries / mailcat state match the deny arm, and the
//! audit log records a `confirm_timeout` event).
//!
//! Why we wait real time instead of `tokio::time::pause`: the gate's
//! 60s TTL is a hard-coded `Duration` inside the `rfl chat`
//! subprocess. `tokio::time::pause()` only affects the current tokio
//! runtime, so the integration test cannot accelerate the gate from
//! the outside without a knob that is explicitly out of scope for
//! c40 (no source changes per the row). `RFL_TUI_MAX_LIFETIME` is
//! extended past the TTL so the TUI stays alive long enough for the
//! timeout to fire.

mod common;

use std::process::Command;

use common::m5a_demo_kit::{
    audit_kinds, install_m5a_demo_lock, mailcat_log_path, stub_send_mail_then_text,
    MailcatToolMetaOverrides, OpenAiStub, MAILCAT_CANONICAL,
};
use common::workspace_bin_path::workspace_bin;
use rafaello_core::entry::EntryAuthor;
use rafaello_core::session::SessionStore;
use rusqlite::Connection;
use serial_test::serial;

#[test]
#[serial(rfl_chat)]
fn rfl_chat_demo_bar_send_mail_timeout() {
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
        MailcatToolMetaOverrides::default(),
    );

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "please email alice")
        .env("RFL_TUI_TEST_CONFIRM_ANSWER", "timeout")
        .env("RFL_TUI_MAX_LIFETIME", "75")
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
    let store = SessionStore::open(&state_dir).expect("reopen SessionStore");
    let stored = store.load_entries().expect("load entries");

    let kinds: Vec<&str> = stored.iter().map(|s| s.entry.kind.as_str()).collect();
    let authors: Vec<&EntryAuthor> = stored.iter().map(|s| &s.entry.metadata.author).collect();
    assert_eq!(
        kinds,
        vec!["text", "tool_call", "tool_result"],
        "unexpected entry kind sequence; stderr={stderr}\nstored={stored:#?}"
    );
    assert_eq!(
        authors,
        vec![
            &EntryAuthor::User,
            &EntryAuthor::Assistant,
            &EntryAuthor::Tool,
        ],
        "unexpected author sequence"
    );

    // Entry-level: the agent loop persists the synthetic deny
    // tool_result with `ok=false` + `call_id` pinned to the held
    // tool_request id (per c22's
    // `gate_synthetic_deny_persists_through_agent_loop`). The
    // §CG4a wire-level shape (taint, error, in_reply_to) is
    // bus-only and covered by
    // `gate_synthesises_deny_tool_result_with_pinned_shape` —
    // not re-asserted here.
    let tool_result = &stored[2].entry;
    assert_eq!(
        tool_result.payload["ok"].as_bool(),
        Some(false),
        "expected tool_result.ok=false on timeout; entry={tool_result:#?}"
    );
    assert!(
        tool_result
            .payload
            .get("call_id")
            .and_then(|v| v.as_str())
            .is_some(),
        "expected tool_result.call_id set to held tool_request id; entry={tool_result:#?}"
    );

    let log_path = mailcat_log_path(project_root);
    let mailcat_empty = !log_path.exists()
        || std::fs::metadata(&log_path)
            .map(|m| m.len() == 0)
            .unwrap_or(true);
    assert!(
        mailcat_empty,
        "expected mailcat.log empty/absent on timeout; path={log_path:?}"
    );

    let conn = Connection::open(state_dir.join("session.sqlite")).expect("open audit sqlite");
    let kinds = audit_kinds(&conn);
    assert!(
        kinds.contains(&"confirm_request".to_string()),
        "audit missing confirm_request; got {kinds:?}"
    );
    assert!(
        kinds.contains(&"confirm_timeout".to_string()),
        "audit missing confirm_timeout; got {kinds:?}"
    );

    let _ = MAILCAT_CANONICAL;
}
