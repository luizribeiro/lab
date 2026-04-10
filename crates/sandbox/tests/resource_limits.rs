//! Resource-limit tests: verify that `SandboxBuilder` rlimit methods
//! are enforced in the child process via `setrlimit`.

mod common;

use std::process::Stdio;

use capsa_sandbox::Sandbox;

use common::{probe_binary, run_probe};

#[test]
fn max_open_files_prevents_child_from_opening_beyond_limit() {
    let probe = probe_binary();

    // Allow 16 fds total. The child starts with stdin/stdout/stderr (3)
    // plus a few internal fds, so asking for 20 opens should fail.
    // /dev/null is the target of open-many-fds; the sandbox must allow it.
    let (mut cmd, _sandbox) = Sandbox::builder()
        .max_open_files(16)
        .read_only_path("/dev/null")
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("open-many-fds")
        .arg("20")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "probe should have failed opening 20 fds with a limit of 16"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to open fd"),
        "expected 'failed to open fd' in stderr, got: {stderr}"
    );
}

#[test]
fn max_open_files_allows_child_within_limit() {
    let probe = probe_binary();

    let (mut cmd, _sandbox) = Sandbox::builder()
        .max_open_files(64)
        .read_only_path("/dev/null")
        .build(&probe)
        .expect("build sandbox");

    cmd.arg("open-many-fds")
        .arg("10")
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd.status().expect("spawn probe");
    assert!(
        status.success(),
        "probe should succeed opening 10 fds with a limit of 64"
    );
}

#[test]
fn child_sees_rlimit_nofile_via_getrlimit() {
    let builder = Sandbox::builder().max_open_files(64);
    assert!(
        run_probe(builder, &["check-rlimit", "nofile", "64"]),
        "child should see RLIMIT_NOFILE=64 via getrlimit"
    );
}

#[test]
fn disable_core_dumps_sets_zero_limit() {
    use std::process::Command;

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg("[ \"$(ulimit -c)\" = \"0\" ]")
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    #[allow(clippy::unnecessary_cast)]
    let rlimits = vec![(libc::RLIMIT_CORE as i32, 0)];
    capsa_sandbox::configure_rlimits(&mut cmd, rlimits).expect("configure_rlimits");

    let status = cmd.status().expect("spawn shell");
    assert!(
        status.success(),
        "ulimit -c should report 0 after disable_core_dumps()"
    );
}
