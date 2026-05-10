mod common;

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use nix::fcntl::{Flock, FlockArg};

#[test]
fn locked_session_unknown_holder_errors() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().join(".rafaello").join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    let lock_path = state_dir.join("session.lock");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(nix::libc::O_CLOEXEC)
        .open(&lock_path)
        .unwrap();
    let _flock = Flock::lock(file, FlockArg::LockExclusiveNonblock)
        .map_err(|(_, e)| e)
        .expect("flock empty lock file");

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
        stderr.contains("unknown process"),
        "stderr missing 'unknown process': {stderr}"
    );
}
