//! c40 — Scope §"Demo bar" §Negative 2: `always_allow_session` clears
//! on `rfl chat` restart.
//!
//! Two invocations of `rfl chat` in the same project tempdir (same
//! SQLite, same lock — but each invocation gets a fresh in-memory
//! `UserGrants`):
//!
//! 1. First run answers `always_allow_session`. mailcat.log gains one
//!    entry; audit records `confirm_allowed_with_session_grant` and
//!    `grant_added`.
//! 2. Second run answers `deny` after a 10ms delay (pi-1 N-6: the
//!    round-1 wording said "unset" while also setting the env vars;
//!    clarified here). Because `UserGrants` is in-memory the grant
//!    from run 1 is gone, so the gate must prompt again: a fresh
//!    `confirm_request` audit entry appears and mailcat.log is
//!    unchanged from run 1.

mod common;

use std::path::Path;
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
fn rfl_chat_always_allow_session_clears_on_restart() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-openai");
    let _ = workspace_bin("rfl-mailcat");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();

    // ---- Run 1: always_allow_session ----
    let stub1 = OpenAiStub::start(stub_send_mail_then_text("Email sent to alice."));
    install_m5a_demo_lock(
        project_root,
        &stub1.endpoint(),
        MailcatToolMetaOverrides::default(),
    );

    let output1 = run_chat(project_root, "always_allow_session", "0");
    drop(stub1);
    let stderr1 = String::from_utf8_lossy(&output1.stderr);
    assert!(
        output1.status.success(),
        "run1: expected zero exit; stderr={stderr1}"
    );

    let state_dir = project_root.join(".rafaello").join("state");
    let log_path = mailcat_log_path(project_root);
    let run1_log = std::fs::read_to_string(&log_path).expect("run1: read mailcat.log");
    let run1_lines: Vec<&str> = run1_log.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        run1_lines.len(),
        1,
        "run1: expected one mailcat.log line; got {run1_lines:#?}"
    );

    let conn = Connection::open(state_dir.join("session.sqlite")).expect("run1: open audit sqlite");
    let run1_kinds = audit_kinds(&conn);
    assert!(
        run1_kinds.contains(&"confirm_request".to_string()),
        "run1: audit missing confirm_request; got {run1_kinds:?}"
    );
    assert!(
        run1_kinds.contains(&"confirm_allowed_with_session_grant".to_string()),
        "run1: audit missing confirm_allowed_with_session_grant; got {run1_kinds:?}"
    );
    assert!(
        run1_kinds.contains(&"grant_added".to_string()),
        "run1: audit missing grant_added; got {run1_kinds:?}"
    );
    drop(conn);

    let run1_confirm_request_count = run1_kinds
        .iter()
        .filter(|k| *k == "confirm_request")
        .count();

    // ---- Run 2: deny after 10ms in a fresh `rfl chat` ----
    // Re-install the lock so the openai endpoint points at the new
    // stub port; the rest of the project state (SQLite, plugin install
    // dirs) is reused as-is.
    let stub2 = OpenAiStub::start(stub_send_mail_then_text(
        "Understood; I will not send the email.",
    ));
    install_m5a_demo_lock(
        project_root,
        &stub2.endpoint(),
        MailcatToolMetaOverrides::default(),
    );

    let output2 = run_chat(project_root, "deny", "10");
    drop(stub2);
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(
        output2.status.success(),
        "run2: expected zero exit; stderr={stderr2}"
    );

    let run2_log = std::fs::read_to_string(&log_path).expect("run2: read mailcat.log");
    assert_eq!(
        run2_log, run1_log,
        "run2: mailcat.log changed (deny should not append); before={run1_log:?}, after={run2_log:?}"
    );

    let conn = Connection::open(state_dir.join("session.sqlite")).expect("run2: open audit sqlite");
    let run2_kinds = audit_kinds(&conn);
    let run2_confirm_request_count = run2_kinds
        .iter()
        .filter(|k| *k == "confirm_request")
        .count();
    assert!(
        run2_confirm_request_count > run1_confirm_request_count,
        "run2: expected a fresh confirm_request (gate must prompt again — UserGrants is per-session); \
         run1_count={run1_confirm_request_count}, run2_count={run2_confirm_request_count}; kinds={run2_kinds:?}"
    );
    assert!(
        run2_kinds.contains(&"confirm_denied".to_string()),
        "run2: audit missing confirm_denied; got {run2_kinds:?}"
    );
}

fn run_chat(project_root: &Path, answer: &str, delay_ms: &str) -> std::process::Output {
    Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "please email alice")
        .env("RFL_TUI_TEST_CONFIRM_ANSWER", answer)
        .env("RFL_TUI_TEST_CONFIRM_DELAY_MS", delay_ms)
        .env("RFL_TUI_MAX_LIFETIME", "10")
        .env("LITELLM_API_KEY", "sk-test-demo-bar")
        .output()
        .expect("spawn rfl chat")
}
