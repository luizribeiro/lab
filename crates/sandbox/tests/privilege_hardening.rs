//! Privilege-hardening tests for `no_new_privs` and capability
//! allowlisting.
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

// ── capability allowlist ────────────────────────────────────

#[test]
fn default_drops_all_capabilities() {
    let probe = probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder().build(&probe).expect("build sandbox");

    if cfg!(target_os = "linux") {
        cmd.arg("check-has-cap")
            .arg("0")
            .stderr(Stdio::piped())
            .stdout(Stdio::inherit());

        let output = cmd.output().expect("spawn probe");
        assert!(
            !output.status.success(),
            "CAP_CHOWN should be dropped by default; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        cmd.arg("can-stat")
            .arg("/dev/null")
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        let status = cmd.status().expect("spawn probe");
        assert!(status.success(), "capability dropping is a no-op on macOS");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn allow_capability_retains_it_in_bounding_set() {
    use capsa_sandbox::Capability;

    let probe = probe_binary();
    let cap_num = u32::from(Capability::NetRaw).to_string();

    let (mut cmd, _sandbox) = Sandbox::builder()
        .allow_capability(Capability::NetRaw)
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("check-has-cap")
        .arg(&cap_num)
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        output.status.success(),
        "CAP_NET_RAW should be retained in bounding set; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn allow_capability_retains_it_in_effective_set() {
    use capsa_sandbox::Capability;

    let probe = probe_binary();
    let cap_num = u32::from(Capability::NetRaw).to_string();

    let (mut cmd, _sandbox) = Sandbox::builder()
        .allow_capability(Capability::NetRaw)
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("check-has-effective-cap")
        .arg(&cap_num)
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        output.status.success(),
        "CAP_NET_RAW should be retained in effective set; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn non_allowed_capability_is_dropped() {
    use capsa_sandbox::Capability;

    let probe = probe_binary();
    let sys_admin_num = u32::from(Capability::SysAdmin).to_string();

    let (mut cmd, _sandbox) = Sandbox::builder()
        .allow_capability(Capability::NetRaw)
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("check-has-cap")
        .arg(&sys_admin_num)
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "CAP_SYS_ADMIN should be dropped when only CAP_NET_RAW is allowed"
    );
}
