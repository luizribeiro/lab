mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;

#[test]
fn frontend_post_ready_nonzero_exit_errors() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-bus-fixture");

    let tmp = tempfile::tempdir().unwrap();
    write_stub_lock(tmp.path());

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(tmp.path())
        .env("RFL_TUI_PATH", workspace_bin("rfl-bus-fixture"))
        .env("RFL_FIXTURE_MODE", "signal_ready_then_exit_n")
        .env("RFL_FIXTURE_EXIT_CODE", "7")
        .env("RFL_FIXTURE_MAX_LIFETIME", "10")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains("FrontendExitedAbnormally"),
        "stderr missing FrontendExitedAbnormally: {stderr}"
    );
    assert!(stderr.contains("7"), "stderr missing exit code 7: {stderr}");
}
