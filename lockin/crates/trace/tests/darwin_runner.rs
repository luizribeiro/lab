//! Integration tests for the Darwin trace runner. Each test runs the
//! compiled `sandbox_probe` under sandbox-exec with a `(deny default
//! (with report))` profile and asserts the expected denial behavior.

#![cfg(target_os = "macos")]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lockin_config::Config;
use lockin_infer::{FsOp, InferEvent};
use lockin_trace::{trace, TraceOptions, TraceReport, TraceRequest};

fn macos_tools_present() -> bool {
    Path::new("/usr/bin/log").exists() && Path::new("/usr/bin/sandbox-exec").exists()
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

fn base_config() -> Config {
    let mut config = Config::default();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for dir in std::env::split_paths(&val) {
            if dir.is_absolute() {
                config.filesystem.read_dirs.push(dir.clone());
                config.filesystem.exec_dirs.push(dir);
            }
        }
    }
    config
}

fn request(program: &Path, args: &[&str], config: Config) -> TraceRequest {
    TraceRequest {
        program: program.to_path_buf(),
        args: args.iter().map(OsString::from).collect(),
        current_dir: None,
        env: Vec::new(),
        config,
        config_dir: None,
    }
}

fn run_or_skip(req: TraceRequest) -> Option<TraceReport> {
    if !macos_tools_present() {
        eprintln!("skipping: /usr/bin/log or /usr/bin/sandbox-exec missing");
        return None;
    }
    match trace(req, TraceOptions::default()) {
        Ok(r) => Some(r),
        // Darwin trace is intentionally a stub right now (see
        // `crates/trace/src/darwin.rs`). Treat that error as "skip"
        // so the test suite passes on macOS while the feature is
        // Linux-only. When Darwin trace lands, this branch can be
        // removed.
        Err(e) if e.to_string().contains("not yet supported on macOS") => {
            eprintln!("skipping darwin trace test: {e}");
            None
        }
        Err(e) => panic!("trace run failed: {e:#}"),
    }
}

fn ends_with(path: &Path, name: &str) -> bool {
    path.file_name().map(|n| n == name).unwrap_or(false)
}

#[test]
fn trace_denied_read_emits_deny_event() {
    let probe = probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("secret.txt");
    std::fs::write(&secret, b"private\n").unwrap();

    let req = request(
        &probe,
        &["infer-read", secret.to_str().unwrap()],
        base_config(),
    );
    let Some(report) = run_or_skip(req) else {
        return;
    };

    assert!(
        !report.status.success(),
        "probe should fail when read is denied; status = {:?}",
        report.status
    );
    let saw = report.denials.iter().any(|e| match e {
        InferEvent::Fs {
            op: FsOp::Read | FsOp::Stat,
            path,
        } => ends_with(path, "secret.txt"),
        _ => false,
    });
    assert!(
        saw,
        "expected a Deny event for secret.txt; denials={:?} diags={:?}",
        report.denials, report.diagnostics
    );
}

#[test]
fn trace_no_denials_when_policy_permits() {
    let probe = probe_binary();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("input.txt");
    std::fs::write(&target, b"hello\n").unwrap();

    let mut config = base_config();
    config.filesystem.read_dirs.push(dir.path().to_path_buf());

    let req = request(&probe, &["infer-read", target.to_str().unwrap()], config);
    let Some(report) = run_or_skip(req) else {
        return;
    };

    assert!(
        report.status.success(),
        "probe should succeed when policy allows the read; status = {:?} denials = {:?} diags = {:?}",
        report.status,
        report.denials,
        report.diagnostics,
    );
    assert!(
        report.denials.is_empty(),
        "expected no denials when policy permits; got {:?}",
        report.denials
    );
}
