mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn relative_project_root_canonicalises() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    let canonical = tmp.path().canonicalize().unwrap();

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(".")
        .current_dir(&canonical)
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_MAX_LIFETIME", "2")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!("rfl-tui: project-root={}", canonical.display());
    assert!(
        stderr.contains(&expected),
        "stderr missing {expected:?}: {stderr}"
    );
}
