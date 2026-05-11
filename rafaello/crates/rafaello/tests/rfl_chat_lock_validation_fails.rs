mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_chat_lock_validation_fails() {
    let _ = workspace_bin("rfl");

    let tmp = tempfile::tempdir().unwrap();
    let lock_toml = r#"
[session]
tool_owner = { "Bad!Name" = "scope:plug" }
"#;
    std::fs::write(tmp.path().join("rafaello.lock"), lock_toml).expect("write lock");

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
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains("LockValidation"),
        "stderr missing LockValidation: {stderr}"
    );
}
