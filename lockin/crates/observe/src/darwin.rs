//! Darwin observation transport backed by `sandbox-exec` + `log stream`.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context};

use crate::parse::seatbelt::{parse_access_message, SeatbeltParseOutcome};
use crate::{AccessEvent, ObserveOptions, ObservedRun};

const LOG_BIN: &str = "/usr/bin/log";
const SANDBOX_EXEC_BIN: &str = "/usr/bin/sandbox-exec";

/// Time to let `log stream` warm up before launching the target.
const LOG_STREAM_WARMUP: Duration = Duration::from_millis(250);
/// Time to wait after the target exits to drain trailing events from `log stream`.
const LOG_STREAM_GRACE: Duration = Duration::from_millis(500);

pub fn observe_with<F>(options: ObserveOptions<'_>, factory: F) -> anyhow::Result<ObservedRun>
where
    F: FnOnce(lockin::SandboxBuilder) -> anyhow::Result<lockin::SandboxedCommand>,
{
    if !std::path::Path::new(LOG_BIN).exists() {
        return Err(anyhow!("{LOG_BIN} not found; not a macOS host?"));
    }
    if !std::path::Path::new(SANDBOX_EXEC_BIN).exists() {
        return Err(anyhow!("{SANDBOX_EXEC_BIN} not found; not a macOS host?"));
    }

    let run_id = format!("lockin-run-{}", uuid::Uuid::new_v4());
    let mut log_stream = start_log_stream(&run_id)?;
    thread::sleep(LOG_STREAM_WARMUP);

    let builder = lockin::SandboxBuilder::new()
        .observation(crate::observation_mode(options.kind, run_id.clone()));
    let cmd = factory(builder)?;
    let status = crate::supervise(cmd, options.runtime)?;

    thread::sleep(LOG_STREAM_GRACE);
    let lines = log_stream.stop_and_collect();
    let (events, diagnostics) = parse_log_lines(lines, &run_id);

    Ok(ObservedRun {
        status,
        events,
        diagnostics,
    })
}

struct LogStream {
    child: ChildGuard,
    reader_thread: Option<thread::JoinHandle<()>>,
    lines: Arc<Mutex<Vec<String>>>,
}

fn start_log_stream(run_id: &str) -> anyhow::Result<LogStream> {
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

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let lines_for_thread = Arc::clone(&lines);
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(log_stdout);
        for line in reader.lines().map_while(std::io::Result::ok) {
            lines_for_thread.lock().unwrap().push(line);
        }
    });

    Ok(LogStream {
        child: ChildGuard(Some(log_child)),
        reader_thread: Some(reader_thread),
        lines,
    })
}

impl LogStream {
    fn stop_and_collect(&mut self) -> Vec<String> {
        // Killing log stream EOFs its stdout, which lets the reader thread exit.
        if let Some(mut c) = self.child.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
        std::mem::take(&mut *self.lines.lock().unwrap())
    }
}

fn parse_log_lines(
    raw_lines: Vec<String>,
    run_id: &str,
) -> (Vec<AccessEvent>, Vec<crate::InferDiagnostic>) {
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
        match parse_access_message(message, run_id) {
            SeatbeltParseOutcome::Event(ae) => events.push(ae),
            SeatbeltParseOutcome::Skip => {}
            SeatbeltParseOutcome::Unsupported { event, diagnostic } => {
                events.push(event);
                diagnostics.push(diagnostic);
            }
            SeatbeltParseOutcome::Malformed(d) => {
                diagnostics.push(d);
            }
        }
    }
    (events, diagnostics)
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
