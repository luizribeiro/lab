mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::session::SessionStore;

#[test]
fn locked_session_errors_with_holder_pid() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    write_stub_lock(tmp.path());
    let state_dir = tmp.path().join(".rafaello").join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    let _holder = SessionStore::open(&state_dir).expect("holder open");

    let our_pid = std::process::id();
    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(tmp.path())
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_MAX_LIFETIME", "2")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit, got {:?}; stderr={stderr}",
        output.status
    );
    assert!(
        stderr.contains(&format!("pid {our_pid}")),
        "stderr missing holder pid {our_pid}: {stderr}"
    );
}
