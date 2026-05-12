//! c23 / pi-1 B-5 — startup ordering acceptance: the tool-schema
//! catalog must be built before the first plugin supervisor spawn,
//! since `core.tools_list` (served by per-connection `CoreService`)
//! resolves against the catalog and the provider issues that call
//! during its handshake.
//!
//! `run_chat` records the startup event sequence through
//! `rafaello::chat::test_ordering_hook`. With
//! `RFL_STARTUP_ORDERING_LOG=<path>` set, the hook mirrors each event
//! to the file — the cross-process drain mechanism this test relies
//! on, since the in-memory queue lives in the child process.

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
fn core_tools_list_registered_before_provider_spawn() {
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
    let catalog_pos = events
        .iter()
        .position(|e| *e == "tool_schema_catalog_built")
        .unwrap_or_else(|| panic!("tool_schema_catalog_built not in log: {events:?}"));
    let spawn_pos = events
        .iter()
        .position(|e| *e == "plugin_supervisor_spawn")
        .unwrap_or_else(|| panic!("plugin_supervisor_spawn not in log: {events:?}"));
    assert!(
        catalog_pos < spawn_pos,
        "tool_schema_catalog_built must precede the first plugin_supervisor_spawn; got: {events:?}"
    );
}
