mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_chat_demo_bar() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    write_stub_lock(project_root);

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_MAX_LIFETIME", "2")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains("NoActiveProvider"),
        "stderr missing NoActiveProvider: {stderr}"
    );
}
