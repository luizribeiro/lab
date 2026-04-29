//! Darwin observation backend backed by `sandbox-exec` + `log stream`.
//!
//! We launch `log stream --predicate 'eventMessage CONTAINS <RUN_ID>'`
//! BEFORE the target process so its events are captured from the start,
//! then run the target under `sandbox-exec` with a profile that allows
//! every access while emitting a Seatbelt report tagged with the
//! per-run UUID. After the target exits we drain any trailing events
//! during a short grace window, then kill the log streamer.
//!
//! A background thread continuously reads `log stream` stdout into a
//! shared Vec<String>; the foreground keeps the pipe drained so log
//! stream never blocks on a full pipe buffer.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use uuid::Uuid;

use crate::backend::{BackendReport, InferBackend, InferRequest};
use crate::parse::seatbelt::{parse_message, SeatbeltParseOutcome};

const LOG_BIN: &str = "/usr/bin/log";
const SANDBOX_EXEC_BIN: &str = "/usr/bin/sandbox-exec";
const SEATBELT_PROFILE: &str =
    r#"(version 1) (allow default (with report) (with message (param "RUN_ID")))"#;

/// Time to let `log stream` warm up before launching the target. There's
/// no clean ready-marker; this is empirically enough but is the source
/// of any "missed first events" flakiness.
const LOG_STREAM_WARMUP: Duration = Duration::from_millis(250);
/// Time to wait after the target exits to drain trailing events from
/// `log stream` before we kill it.
const LOG_STREAM_GRACE: Duration = Duration::from_millis(500);

/// Darwin observation backend.
pub struct DarwinBackend;

impl InferBackend for DarwinBackend {
    fn run(&self, request: &InferRequest) -> Result<BackendReport> {
        run(request)
    }
}

/// Run a program under `sandbox-exec` and collect events from `log
/// stream` filtered by a per-run UUID.
pub fn run(request: &InferRequest) -> Result<BackendReport> {
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

    let mut target = Command::new(SANDBOX_EXEC_BIN);
    target
        .arg("-D")
        .arg(format!("RUN_ID={run_id}"))
        .arg("-p")
        .arg(SEATBELT_PROFILE)
        .arg(&request.program)
        .args(&request.args);

    if let Some(dir) = &request.current_dir {
        target.current_dir(dir);
    }
    for (k, v) in &request.env {
        target.env(k, v);
    }

    let target_child = target
        .spawn()
        .context("failed to spawn sandbox-exec target")?;
    let mut target_guard = ChildGuard(Some(target_child));

    let mut child = target_guard.0.take().expect("child still owned by guard");
    let status = child.wait().context("wait on sandbox-exec child failed")?;

    thread::sleep(LOG_STREAM_GRACE);

    // Killing log stream EOFs its stdout, which lets the reader thread exit.
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
            Err(_) => continue, // log stream prints a banner line; ignore non-JSON.
        };
        let Some(message) = value.get("eventMessage").and_then(|v| v.as_str()) else {
            continue;
        };
        match parse_message(message, &run_id) {
            SeatbeltParseOutcome::Event(ev) => events.push(ev),
            SeatbeltParseOutcome::Skip => {}
            SeatbeltParseOutcome::Unsupported(d) | SeatbeltParseOutcome::Malformed(d) => {
                diagnostics.push(d);
            }
        }
    }
    Ok(BackendReport {
        status,
        events,
        diagnostics,
    })
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
