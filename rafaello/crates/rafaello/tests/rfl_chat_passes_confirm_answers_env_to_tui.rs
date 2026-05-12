//! §TUI-MA2 — `RFL_TUI_TEST_CONFIRM_ANSWERS` is on the rfl env allowlist
//! and therefore reaches the spawned `rfl-tui` process.
//!
//! Test seam: §TUI-MA1's pinned mutual-exclusion startup error fires only
//! when BOTH `RFL_TUI_TEST_CONFIRM_ANSWER` (already allowlisted in m5a)
//! AND `RFL_TUI_TEST_CONFIRM_ANSWERS` (this commit's new allowlist entry)
//! make it into the child env. Setting both in the outer `rfl chat`
//! process and observing the pinned error in forwarded TUI stderr is a
//! sufficient witness that the new allowlist entry is wired up.
//!
//! Drives the m5a fixture lock per c19's row (m5b lock not landed until c22).

mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;

const PINNED_MUTEX_ERR: &str =
    "RFL_TUI_TEST_CONFIRM_ANSWER and RFL_TUI_TEST_CONFIRM_ANSWERS are mutually exclusive; \
     set one or the other";

#[test]
fn rfl_chat_passes_confirm_answers_env_to_tui() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    write_stub_lock(tmp.path());

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(tmp.path())
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_MAX_LIFETIME", "2")
        .env("RFL_TUI_TEST_CONFIRM_ANSWER", "allow")
        .env("RFL_TUI_TEST_CONFIRM_ANSWERS", "allow,deny")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains(PINNED_MUTEX_ERR),
        "stderr missing pinned mutual-exclusion error \
         (would indicate RFL_TUI_TEST_CONFIRM_ANSWERS is not in the rfl env allowlist); \
         stderr={stderr}"
    );
}
