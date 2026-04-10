//! Privilege-hardening tests for `no_new_privs` and capability
//! allowlisting.
//!
//! These features only exist on Linux; on macOS the builder methods
//! are absent and the tests are compiled out.

#[cfg(target_os = "linux")]
mod common;

// ── no_new_privs ────────────────────────────────────────────

#[cfg(target_os = "linux")]
#[test]
fn no_new_privs_is_enabled_by_default() {
    use std::process::Stdio;

    use capsa_sandbox::Sandbox;

    let probe = common::probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder().build(&probe).expect("build sandbox");

    cmd.arg("check-no-new-privs")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        output.status.success(),
        "no_new_privs should be set by default on Linux; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn no_new_privs_can_be_disabled() {
    use std::process::Stdio;

    use capsa_sandbox::Sandbox;

    let probe = common::probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder()
        .no_new_privs(false)
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("check-no-new-privs")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "no_new_privs should NOT be set when disabled"
    );
}

// ── capability allowlist ────────────────────────────────────

#[cfg(target_os = "linux")]
#[test]
fn default_drops_all_capabilities() {
    use std::process::Stdio;

    use capsa_sandbox::Sandbox;

    let probe = common::probe_binary();
    let (mut cmd, _sandbox) = Sandbox::builder().build(&probe).expect("build sandbox");

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
}

#[cfg(target_os = "linux")]
#[test]
fn allow_capability_retains_it_in_bounding_set() {
    use std::process::Stdio;

    use capsa_sandbox::Capability;
    use capsa_sandbox::Sandbox;

    let probe = common::probe_binary();
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
    use std::process::Stdio;

    use capsa_sandbox::Capability;
    use capsa_sandbox::Sandbox;

    let probe = common::probe_binary();
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
    use std::process::Stdio;

    use capsa_sandbox::Capability;
    use capsa_sandbox::Sandbox;

    let probe = common::probe_binary();
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
