//! Integration tests for the Linux observation transport. Each test
//! spawns the compiled `sandbox_probe` under `syd -x` via `observe_with`
//! and asserts the relevant event class shows up in the captured stream.
//!
//! Skipped silently when `LOCKIN_SYD_PATH` is unset (e.g. on a developer
//! machine outside the devenv shell).

#![cfg(target_os = "linux")]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lockin_observe::{FsOp, InferEvent, ObservationKind, ObserveOptions, ObservedRun};

fn syd_available() -> bool {
    std::env::var_os("LOCKIN_SYD_PATH").is_some()
}

fn probe_binary() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../target/debug/sandbox_probe");
    assert!(
        p.exists(),
        "sandbox_probe not found at {} — run `cargo build` first",
        p.display()
    );
    p.canonicalize().unwrap()
}

struct TestRequest {
    program: PathBuf,
    args: Vec<OsString>,
}

fn request(program: &Path, args: &[&str]) -> TestRequest {
    TestRequest {
        program: program.to_path_buf(),
        args: args.iter().map(OsString::from).collect(),
    }
}

fn run_or_skip(req: &TestRequest) -> Option<ObservedRun> {
    if !syd_available() {
        eprintln!("skipping: LOCKIN_SYD_PATH not set");
        return None;
    }
    Some(
        lockin_observe::observe_with(
            ObserveOptions::new(ObservationKind::InferAllowAll),
            |builder| {
                let mut cmd = builder.command(&req.program)?;
                cmd.args(&req.args);
                cmd.stdin(std::process::Stdio::null());
                Ok(cmd)
            },
        )
        .expect("observe_with"),
    )
}

fn ends_with(path: &Path, name: &str) -> bool {
    path.file_name().map(|n| n == name).unwrap_or(false)
}

#[test]
fn infer_read_emits_read_or_stat_event_for_target_path() {
    let probe = probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("input.txt");
    std::fs::write(&target, b"hello\n").unwrap();
    let req = request(&probe, &["infer-read", target.to_str().unwrap()]);
    let Some(report) = run_or_skip(&req) else {
        return;
    };
    assert!(report.status.success(), "probe failed: {:?}", report.status);
    let saw = report.events.iter().any(|ae| match &ae.event {
        InferEvent::Fs {
            op: FsOp::Read | FsOp::Stat,
            path,
        } => ends_with(path, "input.txt"),
        _ => false,
    });
    assert!(
        saw,
        "no Read/Stat event for input.txt; events={:?} diags={:?}",
        report.events, report.diagnostics
    );
}

#[test]
fn infer_write_emits_write_or_create_event_for_target_path() {
    let probe = probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.txt");
    let req = request(&probe, &["infer-write", target.to_str().unwrap()]);
    let Some(report) = run_or_skip(&req) else {
        return;
    };
    assert!(report.status.success(), "probe failed: {:?}", report.status);
    let saw = report.events.iter().any(|ae| match &ae.event {
        InferEvent::Fs {
            op: FsOp::Write | FsOp::Create,
            path,
        } => ends_with(path, "out.txt"),
        _ => false,
    });
    assert!(
        saw,
        "no Write/Create event for out.txt; events={:?} diags={:?}",
        report.events, report.diagnostics
    );
}

#[test]
fn infer_exec_emits_exec_event_for_probe_binary() {
    let probe = probe_binary();
    let probe_str = probe.to_string_lossy().into_owned();
    let req = request(&probe, &["infer-exec", &probe_str, "infer-noop"]);
    let Some(report) = run_or_skip(&req) else {
        return;
    };
    assert!(report.status.success(), "probe failed: {:?}", report.status);
    let saw = report.events.iter().any(
        |ae| matches!(&ae.event, InferEvent::Exec { path } if ends_with(path, "sandbox_probe")),
    );
    assert!(
        saw,
        "no Exec event for sandbox_probe; events={:?}",
        report.events
    );
}

#[test]
fn missing_target_path_propagates_nonzero_exit() {
    let probe = probe_binary();
    let req = request(
        &probe,
        &["infer-read", "/nonexistent/lockin-infer-target-path"],
    );
    let Some(report) = run_or_skip(&req) else {
        return;
    };
    assert!(
        !report.status.success(),
        "expected non-zero exit, got {:?}",
        report.status
    );
}
