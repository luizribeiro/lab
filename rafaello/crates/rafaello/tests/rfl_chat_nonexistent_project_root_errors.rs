mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn nonexistent_project_root_errors() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    let bogus = tmp.path().join("does").join("not").join("exist");

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(&bogus)
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
        stderr.contains("ProjectRootInvalid"),
        "stderr missing ProjectRootInvalid: {stderr}"
    );
}
