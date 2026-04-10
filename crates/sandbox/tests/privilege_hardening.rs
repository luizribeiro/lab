//! Privilege-hardening tests for `no_new_privs` and
//! `drop_all_capabilities`.
//!
//! On Linux these exercise the actual prctl/capset paths. On macOS
//! both features are no-ops, so the tests verify compilation and that
//! the builder methods run without error.

mod common;

use std::process::Stdio;

use capsa_sandbox::Sandbox;

use common::probe_binary;

// ── no_new_privs ────────────────────────────────────────────

#[test]
fn no_new_privs_is_enabled_by_default() {
    // The builder defaults to no_new_privs=true. On Linux the child
    // can verify via /proc/self/status; on macOS this is a no-op so
    // we just verify the sandbox runs successfully.
    let probe = probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder().build(&probe).expect("build sandbox");

    if cfg!(target_os = "linux") {
        cmd.arg("check-no-new-privs")
            .stderr(Stdio::piped())
            .stdout(Stdio::inherit());

        let output = cmd.output().expect("spawn probe");
        assert!(
            output.status.success(),
            "no_new_privs should be set by default on Linux; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        // macOS: just verify the sandbox builds and runs a trivial
        // command without error.
        cmd.arg("can-stat")
            .arg("/dev/null")
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        let status = cmd.status().expect("spawn probe");
        assert!(
            status.success(),
            "no-op no_new_privs should not break macOS"
        );
    }
}

#[test]
fn no_new_privs_can_be_disabled() {
    let probe = probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder()
        .no_new_privs(false)
        .build(&probe)
        .expect("build sandbox");

    if cfg!(target_os = "linux") {
        cmd.arg("check-no-new-privs")
            .stderr(Stdio::piped())
            .stdout(Stdio::inherit());

        let output = cmd.output().expect("spawn probe");
        // When disabled, the probe should report that no_new_privs
        // is NOT set (exit non-zero).
        assert!(
            !output.status.success(),
            "no_new_privs should NOT be set when disabled"
        );
    } else {
        cmd.arg("can-stat")
            .arg("/dev/null")
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        let status = cmd.status().expect("spawn probe");
        assert!(status.success());
    }
}

// ── drop_all_capabilities ───────────────────────────────────

#[test]
fn drop_all_capabilities_runs_without_error() {
    let probe = probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder()
        .drop_all_capabilities()
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("can-stat")
        .arg("/dev/null")
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd.status().expect("spawn probe");
    assert!(
        status.success(),
        "drop_all_capabilities should not prevent reading /dev/null"
    );
}
