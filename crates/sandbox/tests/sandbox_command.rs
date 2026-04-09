//! Contract tests for the `Sandbox::command` factory API.
//!
//! These mirror a subset of `sandbox_contract.rs` but drive the probe through
//! `Sandbox::new(spec)?.command(&probe)` + plain `std::process::Command::status`
//! instead of the legacy `spawn_sandboxed` entry point.

mod common;

use common::{probe_binary, TestDir};

fn run_probe_via_command(spec: capsa_sandbox::SandboxSpec, args: &[&str]) -> bool {
    let probe = probe_binary();
    let sandbox = capsa_sandbox::Sandbox::new(spec)
        .unwrap_or_else(|e| panic!("failed to build sandbox: {e}"));

    let status = sandbox
        .command(&probe)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to run sandboxed probe: {e}"));

    status.success()
}

#[test]
fn command_factory_enforces_read_allowlist() {
    let temp = TestDir::new("cmd-read");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");

    std::fs::write(&allowed, b"allowed").expect("write allowed fixture");
    std::fs::write(&sibling, b"sibling").expect("write sibling fixture");

    let mut spec = capsa_sandbox::SandboxSpec::new();
    spec.read_only_paths.push(allowed.clone());

    assert!(run_probe_via_command(
        spec.clone(),
        &["can-read", &allowed.display().to_string()]
    ));
    assert!(!run_probe_via_command(
        spec,
        &["can-read", &sibling.display().to_string()]
    ));
}

#[test]
fn command_factory_enforces_write_allowlist() {
    let temp = TestDir::new("cmd-write");
    let allowed = temp.join("ok.txt");
    let denied = temp.join("nope.txt");
    std::fs::write(&allowed, b"seed").expect("seed allowed file");
    std::fs::write(&denied, b"seed").expect("seed denied file");

    let mut spec = capsa_sandbox::SandboxSpec::new();
    spec.read_write_paths.push(allowed.clone());

    assert!(run_probe_via_command(
        spec.clone(),
        &["can-write", &allowed.display().to_string()]
    ));
    assert!(!run_probe_via_command(
        spec,
        &["can-write", &denied.display().to_string()]
    ));
}

#[test]
fn command_factory_grants_write_to_private_tmp() {
    // The factory should wire up $TMPDIR to the sandbox's private tmp and
    // grant write access there, so normal tempfile operations just work.
    let spec = capsa_sandbox::SandboxSpec::new();
    assert!(run_probe_via_command(spec, &["can-write-temp"]));
}

#[cfg(target_os = "linux")]
#[test]
fn sandbox_new_rejects_allow_network_on_linux() {
    let spec = capsa_sandbox::SandboxSpec::new().allow_network(true);
    let err = capsa_sandbox::Sandbox::new(spec)
        .err()
        .expect("Sandbox::new must reject allow_network=true on Linux");
    let msg = err.to_string();
    assert!(
        msg.contains("network"),
        "error should mention the network conflict, got: {msg}"
    );
}

#[test]
fn sandbox_private_tmp_lives_until_drop() {
    let sandbox =
        capsa_sandbox::Sandbox::new(capsa_sandbox::SandboxSpec::new()).expect("build sandbox");
    let tmp = sandbox.private_tmp().to_path_buf();
    assert!(tmp.is_dir(), "private tmp should exist while sandbox held");

    drop(sandbox);
    assert!(
        !tmp.exists(),
        "private tmp should be removed when sandbox is dropped"
    );
}
