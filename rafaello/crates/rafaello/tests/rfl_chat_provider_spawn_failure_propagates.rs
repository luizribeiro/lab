mod common;

use std::process::Command;

use common::m4_install::{install_demo_layout, InstallOptions};
use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_chat_provider_spawn_failure_propagates() {
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    install_demo_layout(
        tmp.path(),
        InstallOptions {
            provider_executable: false,
            tool_executable: true,
            real_binaries: false,
        },
    );

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
        stderr.contains("ProviderSpawnFailed"),
        "stderr missing ProviderSpawnFailed: {stderr}"
    );
    assert!(
        !stderr.contains("ToolSpawnFailed"),
        "stderr should not mention ToolSpawnFailed: {stderr}"
    );
}
