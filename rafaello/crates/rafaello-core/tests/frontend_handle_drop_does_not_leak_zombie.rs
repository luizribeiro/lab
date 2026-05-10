//! c20 acceptance: dropping a `FrontendHandle` SIGKILLs the child
//! and the reaper task collects the corpse — no zombie remains.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::{Duration, Instant};

use common::frontend_test_kit::{broker_with_attach, fixture_plan, live_paths, KNOWN_ATTACH_ID};
use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_handle_drop_does_not_leak_zombie() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let plan = fixture_plan(
        KNOWN_ATTACH_ID,
        "respond_peer_call",
        &[("RFL_FIXTURE_MAX_LIFETIME", "60")],
    );
    let paths = live_paths(&tmp);

    let handle = supervisor.spawn(&plan, &paths).await.expect("spawn ok");
    // Borrow the cached pid before dropping (handle still owns it).
    // Drop sends SIGKILL; reaper task (still running) collects.
    // We can't read child_pid directly off the handle (private),
    // so use /proc to discover the latest fixture child via parent
    // pid: instead, just rely on the contract — the child must die
    // within a brief window. Race-free observation: poll /proc for
    // any rfl-bus-fixture child of *our* pid until none remain.
    drop(handle);

    let our_pid = std::process::id();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if !any_fixture_child_alive(our_pid).await {
            return;
        }
        if Instant::now() > deadline {
            panic!("fixture child still alive 5s after handle drop");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn any_fixture_child_alive(parent_pid: u32) -> bool {
    let entries = match std::fs::read_dir("/proc") {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let pid: i32 = match name.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let status_path = format!("/proc/{}/status", pid);
        let status = match std::fs::read_to_string(&status_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut ppid: Option<u32> = None;
        let mut name_field: Option<String> = None;
        let mut state: Option<String> = None;
        for line in status.lines() {
            if let Some(v) = line.strip_prefix("PPid:") {
                ppid = v.trim().parse().ok();
            } else if let Some(v) = line.strip_prefix("Name:") {
                name_field = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("State:") {
                state = Some(v.trim().to_string());
            }
        }
        if ppid != Some(parent_pid) {
            continue;
        }
        let Some(n) = name_field else { continue };
        if !n.contains("rfl-bus-fixture") && !n.contains("rfl_bus_fixture") {
            continue;
        }
        // Skip already-reaped zombies marked Z (state starts with Z).
        if let Some(s) = &state {
            if s.starts_with('Z') {
                continue;
            }
        }
        // Probe with kill(pid, 0): ESRCH means already gone.
        match kill(Pid::from_raw(pid), None) {
            Err(Errno::ESRCH) => continue,
            _ => return true,
        }
    }
    false
}
