//! Integration tests for the Linux observation backend. Each test
//! spawns the compiled `sandbox_probe` under `syd -x` via `linux::run`
//! and asserts the relevant event class shows up in the captured stream.
//!
//! Skipped silently when `LOCKIN_SYD_PATH` is unset (e.g. on a developer
//! machine outside the devenv shell).

#![cfg(target_os = "linux")]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lockin_infer::backend::linux;
use lockin_infer::backend::InferRequest;
use lockin_infer::{FsOp, InferEvent};

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

fn request(program: &Path, args: &[&str]) -> InferRequest {
    InferRequest {
        program: program.to_path_buf(),
        args: args.iter().map(OsString::from).collect(),
        current_dir: None,
        env: Vec::new(),
    }
}

fn run_or_skip(req: &InferRequest) -> Option<lockin_infer::backend::BackendReport> {
    if !syd_available() {
        eprintln!("skipping: LOCKIN_SYD_PATH not set");
        return None;
    }
    Some(linux::run(req).expect("backend run"))
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
    let saw = report.events.iter().any(|e| match e {
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
    let saw = report.events.iter().any(|e| match e {
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
    let saw = report
        .events
        .iter()
        .any(|e| matches!(e, InferEvent::Exec { path } if ends_with(path, "sandbox_probe")));
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
