#![cfg(target_os = "linux")]
//! c05 — tmux-driven Ctrl-C regression for `rfl chat`. Mirrors cK5
//! (`rfl_chat_production_tui_input_overlay_e2e.rs`) but trimmed to the
//! Ctrl-C exit path: proves the raw-mode TTY path delivers C-c all the
//! way through crossterm's event stream and the parent/child stack
//! tears down cleanly. Load-bearing regression anchor for D2.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use common::m4_install::{install_demo_layout, InstallOptions};
use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_chat_ctrl_c_quits_cleanly() {
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("tmux not available; skipping c05 Ctrl-C regression test");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    install_demo_layout(
        tmp.path(),
        InstallOptions {
            provider_executable: true,
            tool_executable: true,
            real_binaries: true,
        },
    );

    let rfl = workspace_bin("rfl");
    let tui = workspace_bin("rfl-tui");

    let wrapper = tmp.path().join("rfl-chat-wrapper.sh");
    let log_path = tmp.path().join("rfl-chat.stderr");
    // Why: tmux `new-session` does not propagate arbitrary parent env
    // to the child command; bake RFL_TUI_PATH + TERM into a wrapper
    // and `exec` so the wrapper does not linger as a parent process
    // that would mask `rfl chat`'s exit status.
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\n\
             export RFL_TUI_PATH='{tui}'\n\
             export TERM='xterm-256color'\n\
             exec '{rfl}' chat --project-root '{root}' 2>'{log}'\n",
            tui = tui.display(),
            rfl = rfl.display(),
            root = tmp.path().display(),
            log = log_path.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();

    // Why: Ulid nonce keeps sessions disjoint across concurrent test
    // runs (cargo nextest, retries) — PID alone collides on reuse.
    let session = format!("rfl-c05-ctrlc-{}", ulid::Ulid::new());

    let new = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-x", "100", "-y", "30"])
        .arg(&wrapper)
        .output()
        .expect("spawn tmux new-session");
    assert!(
        new.status.success(),
        "tmux new-session failed: stderr={}",
        String::from_utf8_lossy(&new.stderr)
    );

    let guard = TmuxSessionGuard {
        session: session.clone(),
    };

    poll_for_stderr_line(
        &log_path,
        "frontend-ready-observed",
        Duration::from_secs(30),
    );

    let send = Command::new("tmux")
        .args(["send-keys", "-t", &session, "C-c"])
        .status()
        .expect("tmux send-keys C-c");
    assert!(send.success(), "tmux send-keys C-c failed");

    let exit_deadline = Instant::now() + Duration::from_secs(5);
    while session_alive(&session) {
        if Instant::now() >= exit_deadline {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let alive = session_alive(&session);

    if alive {
        let pane = tmux_capture(&session);
        let stderr = fs::read_to_string(&log_path).unwrap_or_default();
        drop(guard);
        panic!("rfl chat did not exit within 5s of Ctrl-C.\npane:\n{pane}\nstderr:\n{stderr}");
    }

    drop(guard);

    let stderr = fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        !stderr.contains("panicked"),
        "rfl chat stderr contains panic during shutdown:\n{stderr}"
    );
}

struct TmuxSessionGuard {
    session: String,
}

impl Drop for TmuxSessionGuard {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &self.session])
            .output();
    }
}

fn poll_for_stderr_line(log_path: &Path, needle: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if fs::read_to_string(log_path)
            .unwrap_or_default()
            .contains(needle)
        {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn session_alive(session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn tmux_capture(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-t", session, "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).into_owned()
}
