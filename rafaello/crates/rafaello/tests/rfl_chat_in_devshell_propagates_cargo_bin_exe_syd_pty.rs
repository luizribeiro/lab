//! Scope §C3 closer: inside `nix develop .#rafaello --impure`, the
//! plumbing for `CARGO_BIN_EXE_syd-pty` end-to-end is reachable. The
//! lockin-side env injection is proven by the fake-syd tests in
//! `lockin/crates/sandbox/tests/`; here we assert the *rafaello-side
//! preconditions* hold in the devshell process spawned by `nix
//! develop`:
//!
//! * the live `rfl-bus-fixture` binary honors the new
//!   `RFL_BUS_FIXTURE_RECORD_ENV` arm at the top of `main()`,
//! * `LOCKIN_SYD_PATH` is visible to that subprocess (set by c08), and
//! * a `syd-pty` binary exists next to that `syd` — so lockin's
//!   sibling-discovery arm at runtime would resolve a real
//!   `CARGO_BIN_EXE_syd-pty` for every sandboxed child the supervisor
//!   spawns inside the devshell.

#![cfg(target_os = "linux")]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::workspace_bin_path::workspace_bin;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cur = manifest_dir.as_path();
    loop {
        let candidate = cur.join("Cargo.toml");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if text.contains("[workspace]") {
                    return cur.to_path_buf();
                }
            }
        }
        cur = cur
            .parent()
            .unwrap_or_else(|| panic!("no workspace root above {}", manifest_dir.display()));
    }
}

#[test]
fn rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty() {
    if Command::new("nix").arg("--version").output().is_err() {
        eprintln!("skipping: `nix` not available on PATH");
        return;
    }

    let bus_fixture = workspace_bin("rfl-bus-fixture");
    let tmp = tempfile::tempdir().expect("tempdir");
    let record_path = tmp.path().join("fixture-env.json");

    let installable = format!("{}#rafaello", workspace_root().display());

    let output = Command::new("nix")
        .arg("develop")
        .arg(&installable)
        .arg("--impure")
        .arg("--command")
        .arg(&bus_fixture)
        .env("RFL_BUS_FIXTURE_RECORD_ENV", &record_path)
        .env("RFL_FIXTURE_MODE", "exit_immediately")
        .output()
        .expect("invoke nix develop");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        record_path.exists(),
        "fixture never wrote env record\nstderr:\n{stderr}"
    );

    let blob = std::fs::read_to_string(&record_path).expect("read record");
    let parsed: serde_json::Value = serde_json::from_str(&blob).expect("parse env JSON");
    let env = parsed.as_object().expect("env is JSON object");

    let syd = env
        .get("LOCKIN_SYD_PATH")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("LOCKIN_SYD_PATH missing from devshell env: {blob}"));
    let syd_path = Path::new(syd);
    assert!(
        syd_path.is_absolute(),
        "LOCKIN_SYD_PATH must be absolute, got: {syd}"
    );

    let pty = syd_path
        .parent()
        .map(|p| p.join("syd-pty"))
        .expect("syd has a parent dir");
    assert!(
        pty.exists(),
        "syd-pty must exist next to syd for lockin's sibling-discovery arm in the devshell, expected {}",
        pty.display()
    );
}
