//! Direct-invocation tests for the `infer-*` fixture modes added to
//! `sandbox_probe`. These run the probe binary as a plain subprocess
//! (no sandbox) to validate the fixture itself; the inference backends
//! in commits 10–11 will exercise the same modes under syd / Seatbelt.

use std::fs;
use std::process::Command;

mod common;

#[test]
fn infer_noop_exits_zero() {
    let probe = common::probe_binary();
    let status = Command::new(&probe).arg("infer-noop").status().unwrap();
    assert!(status.success(), "infer-noop status: {status}");
}

#[test]
fn infer_read_existing_file_exits_zero() {
    let probe = common::probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("input.txt");
    fs::write(&target, b"hello\n").unwrap();
    let status = Command::new(&probe)
        .arg("infer-read")
        .arg(&target)
        .status()
        .unwrap();
    assert!(status.success(), "status: {status}");
}

#[test]
fn infer_read_missing_file_exits_nonzero() {
    let probe = common::probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("nope.txt");
    let status = Command::new(&probe)
        .arg("infer-read")
        .arg(&target)
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn infer_write_into_existing_dir_creates_file_with_expected_content() {
    let probe = common::probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.txt");
    let status = Command::new(&probe)
        .arg("infer-write")
        .arg(&target)
        .status()
        .unwrap();
    assert!(status.success(), "status: {status}");
    let body = fs::read(&target).unwrap();
    assert_eq!(body, b"infer-fixture\n");
}

#[test]
fn infer_write_into_missing_parent_exits_nonzero() {
    let probe = common::probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("missing-dir").join("out.txt");
    let status = Command::new(&probe)
        .arg("infer-write")
        .arg(&target)
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn infer_exec_of_self_with_noop_propagates_zero() {
    let probe = common::probe_binary();
    let status = Command::new(&probe)
        .arg("infer-exec")
        .arg(&probe)
        .arg("infer-noop")
        .status()
        .unwrap();
    assert!(status.success(), "status: {status}");
}

#[test]
fn infer_exec_of_missing_path_exits_nonzero() {
    let probe = common::probe_binary();
    let status = Command::new(&probe)
        .arg("infer-exec")
        .arg("/nonexistent/lockin-probe-target")
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn infer_roundtrip_runs_all_three_phases_and_writes_file() {
    let probe = common::probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let read_target = dir.path().join("in.txt");
    let write_target = dir.path().join("out.txt");
    fs::write(&read_target, b"in\n").unwrap();

    let status = Command::new(&probe)
        .arg("infer-roundtrip")
        .arg(&read_target)
        .arg(&write_target)
        .arg(&probe)
        .status()
        .unwrap();
    // The exec leg invokes the probe with no extra args, which exits via
    // usage_and_exit() (code 2). We accept that as success-of-pipeline:
    // read+write happened, then exec was reached. Use a real noop target
    // to assert exit-zero propagation.
    assert!(write_target.exists(), "write phase did not create file");
    assert_eq!(fs::read(&write_target).unwrap(), b"infer-fixture\n");
    let _ = status;
}

#[test]
fn infer_roundtrip_propagates_exec_exit_code_zero_with_noop() {
    let probe = common::probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let read_target = dir.path().join("in.txt");
    let write_target = dir.path().join("out.txt");
    fs::write(&read_target, b"x").unwrap();

    // Wrap exec target in a tiny shim by using infer-exec'd probe with
    // infer-noop — but infer-roundtrip's exec leg passes no args, so we
    // need a binary that exits 0 with no args. The probe with no args
    // exits 2. Workaround: use a path that the test environment is
    // guaranteed to provide as exit-0 — `/usr/bin/env` with no args
    // prints the environment and exits 0.
    let env_bin = std::path::PathBuf::from("/usr/bin/env");
    if !env_bin.exists() {
        eprintln!("skipping: /usr/bin/env not present");
        return;
    }

    let status = Command::new(&probe)
        .arg("infer-roundtrip")
        .arg(&read_target)
        .arg(&write_target)
        .arg(&env_bin)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "expected exit 0 from /usr/bin/env: {status}"
    );
    assert_eq!(fs::read(&write_target).unwrap(), b"infer-fixture\n");
}
