//! Darwin trace backend: `sandbox-exec` with the user's policy applied
//! as enforcement allows + a `(deny default (with message (param
//! "RUN_ID")))` catch-all that tags every kernel deny line with the
//! per-run UUID.
//!
//! Mirrors `crates/infer/src/backend/darwin.rs`'s structure — start
//! `log stream` with a RUN_ID predicate, warmup, run target via
//! `SandboxBuilder + supervise_command`, drain trailing events, parse.
//! Returns all parsed events; the runner filters to
//! `AccessAction::Deny`.
//!
//! Apple's `sandbox-exec` does not accept `(with report)` on deny
//! actions, but `(with message ...)` is accepted, and the kernel
//! auto-publishes deny lines to `log stream` regardless — so the
//! tagged denial messages reach our predicate-filtered stream the
//! same way infer's allow-with-report messages do.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use lockin::ObservationMode;
use lockin_observe::parse::seatbelt::{parse_access_message, SeatbeltParseOutcome};
use lockin_observe::{AccessEvent, InferDiagnostic};
use uuid::Uuid;

use crate::runner::TraceRequest;

const LOG_BIN: &str = "/usr/bin/log";
const SANDBOX_EXEC_BIN: &str = "/usr/bin/sandbox-exec";

const LOG_STREAM_WARMUP: Duration = Duration::from_millis(250);
const LOG_STREAM_GRACE: Duration = Duration::from_millis(500);

pub(crate) fn run(
    request: &TraceRequest,
) -> Result<(ExitStatus, Vec<AccessEvent>, Vec<InferDiagnostic>)> {
    if !std::path::Path::new(LOG_BIN).exists() {
        return Err(anyhow!("{LOG_BIN} not found; not a macOS host?"));
    }
    if !std::path::Path::new(SANDBOX_EXEC_BIN).exists() {
        return Err(anyhow!("{SANDBOX_EXEC_BIN} not found; not a macOS host?"));
    }

    let run_id = format!("lockin-run-{}", Uuid::new_v4());

    let mut log_child = Command::new(LOG_BIN)
        .arg("stream")
        .arg("--style")
        .arg("ndjson")
        .arg("--predicate")
        .arg(format!("eventMessage CONTAINS \"{run_id}\""))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn `{LOG_BIN} stream`"))?;
    let log_stdout = log_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("log stream stdout missing"))?;
    let mut log_guard = ChildGuard(Some(log_child));

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let lines_for_thread = Arc::clone(&lines);
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(log_stdout);
        for line in reader.lines().map_while(std::io::Result::ok) {
            lines_for_thread.lock().unwrap().push(line);
        }
    });

    thread::sleep(LOG_STREAM_WARMUP);

    let mut builder = lockin_config::apply_config(&request.config, request.config_dir.as_deref())
        .context("applying user lockin.toml policy")?;
    builder = builder
        .observation(ObservationMode::DenyTraceWithRunId(run_id.clone()))
        .network(request.network);

    let mut cmd = builder
        .command(&request.program)
        .context("building trace sandbox command")?;
    cmd.args(&request.args);
    if let Some(dir) = &request.current_dir {
        cmd.current_dir(dir);
    }
    lockin_config::apply_env(&request.config.env, &mut cmd, std::env::vars_os());
    for (k, v) in &request.env {
        cmd.env(k, v);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("building tokio runtime for trace supervision")?;

    let status = lockin::supervise::supervise_command(cmd, runtime.handle())?;

    thread::sleep(LOG_STREAM_GRACE);

    if let Some(mut c) = log_guard.0.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    let _ = reader_thread.join();

    let raw_lines = std::mem::take(&mut *lines.lock().unwrap());
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    for line in raw_lines {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(message) = value.get("eventMessage").and_then(|v| v.as_str()) else {
            continue;
        };
        match parse_access_message(message, &run_id) {
            SeatbeltParseOutcome::Event(ae) => events.push(ae),
            SeatbeltParseOutcome::Skip => {}
            SeatbeltParseOutcome::Unsupported { event, .. } => {
                // Drop the diagnostic — kernel-emitted unsupported ops
                // (mach-lookup, ipc-posix-shm-write-create, etc.) are
                // routinely chatty and per-line stderr noise drowns the
                // useful signal. The denial event still flows through to
                // the human-readable log.
                events.push(event);
            }
            SeatbeltParseOutcome::Malformed(d) => {
                diagnostics.push(d);
            }
        }
    }

    Ok((status, events, diagnostics))
}

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
