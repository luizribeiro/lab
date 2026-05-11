//! c02 / pi-3 M-4 / pi-5 B-2 — startup ordering acceptance.
//!
//! `run_chat` records the startup event sequence through
//! `rafaello::chat::test_ordering_hook`. When the env var
//! `RFL_STARTUP_ORDERING_LOG` is set, the hook mirrors each event to
//! that file — the cross-process drain mechanism this test uses.
//! Spawning `rfl chat` against the m5a fixture lock (m5b-specific
//! topology is irrelevant for this ordering assertion) we expect to
//! observe `set_audit_writer` strictly before any
//! `plugin_supervisor_spawn` event.

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
fn rfl_chat_calls_set_audit_writer_before_first_plugin_spawn() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-mockprovider");
    let _ = workspace_bin("rfl-readfile");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    write_stub_lock(project_root);

    let log_path = tmp.path().join("startup-ordering.log");

    let mut child = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "hello")
        .env("RFL_TUI_MAX_LIFETIME", "30")
        .env("RFL_STARTUP_ORDERING_LOG", &log_path)
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
    let mut saw_ready = false;
    let mut captured: Vec<String> = Vec::new();
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
            Err(_) => break,
        }
    }
    assert!(
        saw_ready,
        "frontend never signalled ready within 6s; stderr:\n{}",
        captured.join("\n")
    );

    let _ = child.wait().expect("rfl chat wait");
    drop(rx);
    let _ = reader_handle.join();

    let log =
        std::fs::read_to_string(&log_path).expect("startup ordering log file must exist after run");
    let events: Vec<&str> = log.lines().collect();
    let set_pos = events
        .iter()
        .position(|e| *e == "set_audit_writer")
        .unwrap_or_else(|| panic!("set_audit_writer not in log: {events:?}"));
    let spawn_pos = events
        .iter()
        .position(|e| *e == "plugin_supervisor_spawn")
        .unwrap_or_else(|| panic!("plugin_supervisor_spawn not in log: {events:?}"));
    assert!(
        set_pos < spawn_pos,
        "set_audit_writer must precede the first plugin_supervisor_spawn; got: {events:?}"
    );
}
