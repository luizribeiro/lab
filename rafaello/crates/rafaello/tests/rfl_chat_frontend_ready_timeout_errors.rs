mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn frontend_ready_timeout_errors() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-bus-fixture");

    let tmp = tempfile::tempdir().unwrap();

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(tmp.path())
        .env("RFL_TUI_PATH", workspace_bin("rfl-bus-fixture"))
        .env("RFL_FIXTURE_MODE", "hold_silent")
        .env("RFL_FIXTURE_MAX_LIFETIME", "10")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains("FrontendReadyTimeout"),
        "stderr missing FrontendReadyTimeout: {stderr}"
    );
}
