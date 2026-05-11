//! c26 — smoke test for the `rfl chat` wiring (scope §C9–§C12):
//! eager-spawned provider + tool plugins, ReemitRouter, AgentLoop, and
//! TUI all wired up before the user message reaches the bus.
//!
//! Sends `RFL_TUI_TEST_MESSAGE="hello"` (no match in the mockprovider's
//! "what's in <path>" pattern → echo path, no tool round-trip), watches
//! for the two `core.session.entry.finalized` events on the forwarded
//! TUI stderr (user "hello" → assistant "echo: hello"), then SIGINTs
//! the parent `rfl` and asserts a clean zero exit.

#![cfg(target_os = "linux")]

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use serial_test::serial;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;

#[test]
#[serial(rfl_chat)]
fn rfl_chat_eager_spawns_provider_and_tool_then_shuts_down_cleanly() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-mockprovider");
    let _ = workspace_bin("rfl-readfile");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    write_stub_lock(project_root);

    let mut child = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "hello")
        // Safety belt: cap the TUI lifetime well above the 6s gate so
        // SIGINT remains the primary exit path, but a wedged child still
        // unblocks the test.
        .env("RFL_TUI_MAX_LIFETIME", "30")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl chat");

    let stderr = child.stderr.take().expect("stderr piped");
    let (tx, rx) = mpsc::channel::<String>();
    let reader_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(6);
    let mut captured: Vec<String> = Vec::new();
    let mut finalized_count = 0usize;
    let mut signalled = false;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                captured.push(line.clone());
                if line.contains("topic=core.session.entry.finalized") {
                    finalized_count += 1;
                    if finalized_count >= 2 {
                        let _ = signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT);
                        signalled = true;
                        break;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    assert!(
        signalled,
        "did not observe two `core.session.entry.finalized` events within 6s; stderr so far:\n{}",
        captured.join("\n")
    );

    let status = child.wait().expect("rfl chat wait");
    // Drain remaining stderr (best-effort) so the reader thread joins.
    drop(rx);
    let _ = reader_handle.join();

    assert!(
        status.success(),
        "rfl chat should exit 0 after SIGINT; status={status:?}; stderr:\n{}",
        captured.join("\n")
    );
}
