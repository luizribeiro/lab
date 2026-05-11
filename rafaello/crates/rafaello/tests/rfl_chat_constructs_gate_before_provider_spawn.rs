//! c38 / scope §CHAT1 — gate-before-provider-spawn ordering.
//!
//! Drives `rfl chat` against the m4 mockprovider+readfile fixture
//! lock and asserts the five-tree wiring still produces a clean
//! shutdown. The ordering guarantee (gate subscribed before first
//! plugin spawn) is enforced in-process by `run_chat`'s edit:
//! `ConfirmationGate::spawn` is called (subscribes the gate's task
//! internally via `Broker::subscribe_internal`) and only then is
//! `PluginSupervisor::spawn` invoked for the active provider. This
//! test smoke-checks that the new orchestration order does not
//! regress the m4 happy path and shuts down cleanly when the gate
//! is present.

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
fn rfl_chat_constructs_gate_before_provider_spawn() {
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
    let mut saw_ready = false;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                captured.push(line.clone());
                if line.contains("rfl-chat: frontend-ready-observed") {
                    saw_ready = true;
                    let _ = signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(
        saw_ready,
        "frontend never signalled ready within 6s; stderr so far:\n{}",
        captured.join("\n")
    );

    let status = child.wait().expect("rfl chat wait");
    drop(rx);
    let _ = reader_handle.join();

    assert!(
        status.success(),
        "rfl chat should exit 0 after SIGINT; status={status:?}; stderr:\n{}",
        captured.join("\n")
    );
}
