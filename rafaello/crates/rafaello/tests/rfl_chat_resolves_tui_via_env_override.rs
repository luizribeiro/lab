mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;

#[test]
fn resolves_tui_via_env_override() {
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
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit (NoActiveProvider); stderr={stderr}"
    );
    assert!(
        stderr.contains("NoActiveProvider"),
        "stderr missing NoActiveProvider: {stderr}"
    );
    assert!(
        stderr.contains("rfl-chat: frontend-ready-observed"),
        "stderr missing parent ready sentinel: {stderr}"
    );
}
