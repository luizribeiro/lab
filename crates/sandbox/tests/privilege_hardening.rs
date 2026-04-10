//! Privilege-hardening tests for `no_new_privs` and capability
//! allowlisting.
//!
//! These features only exist on Linux; on macOS the builder methods
//! are absent and the tests are compiled out.
//!
//! Tests use `configure_privilege_hardening` directly on a bare
//! `Command` because privilege hardening is applied in the bypass
//! path, not through the sandbox wrapper (syd handles its own
//! privilege posture).

#[cfg(target_os = "linux")]
mod common;

// ── no_new_privs ────────────────────────────────────────────

#[cfg(target_os = "linux")]
#[test]
fn no_new_privs_is_enabled_by_default() {
    use std::collections::HashSet;
    use std::process::{Command, Stdio};

    let probe = common::probe_binary();
    let mut cmd = Command::new(&probe);
    cmd.arg("check-no-new-privs")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    capsa_sandbox::configure_privilege_hardening(&mut cmd, true, HashSet::new())
        .expect("configure privilege hardening");

    let output = cmd.output().expect("spawn probe");
    assert!(
        output.status.success(),
        "no_new_privs should be set when enabled; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn no_new_privs_disabled_leaves_it_unset() {
    use std::collections::HashSet;
    use std::process::{Command, Stdio};

    let probe = common::probe_binary();
    let mut cmd = Command::new(&probe);
    cmd.arg("check-no-new-privs")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    capsa_sandbox::configure_privilege_hardening(&mut cmd, false, HashSet::new())
        .expect("configure privilege hardening");

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "no_new_privs should NOT be set when disabled"
    );
}

// ── capability dropping ─────────────────────────────────────

#[cfg(target_os = "linux")]
#[test]
fn empty_allowlist_zeroes_effective_capabilities() {
    use std::collections::HashSet;
    use std::process::{Command, Stdio};

    let probe = common::probe_binary();
    let mut cmd = Command::new(&probe);
    // Check effective set (not bounding set — bounding set can't be
    // modified by unprivileged users, but effective/permitted CAN
    // always be reduced via capset).
    cmd.arg("check-has-effective-cap")
        .arg("0") // CAP_CHOWN
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    capsa_sandbox::configure_privilege_hardening(&mut cmd, false, HashSet::new())
        .expect("configure privilege hardening");

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "CAP_CHOWN should not be in effective set after empty allowlist; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn privilege_hardening_runs_without_error() {
    use std::collections::HashSet;
    use std::process::{Command, Stdio};

    let probe = common::probe_binary();
    let mut cmd = Command::new(&probe);
    cmd.arg("check-no-new-privs")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    // Apply both NNP + empty cap allowlist — verify the child
    // starts without EPERM or other errors regardless of whether
    // the current user has CAP_SETPCAP for the bounding set.
    capsa_sandbox::configure_privilege_hardening(&mut cmd, true, HashSet::new())
        .expect("configure privilege hardening");

    let output = cmd.output().expect("spawn should succeed");
    assert!(
        output.status.success(),
        "child with NNP + empty cap allowlist should start; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
