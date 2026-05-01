//! Darwin observation transport backed by `sandbox-exec` + `log stream`.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};

use crate::parse::seatbelt::{parse_access_message, SeatbeltParseOutcome};
use crate::{AccessEvent, ObserveOptions, ObservedRun};

const LOG_BIN: &str = "/usr/bin/log";
const SANDBOX_EXEC_BIN: &str = "/usr/bin/sandbox-exec";

const SENTINEL_TIMEOUT: Duration = Duration::from_secs(5);
static LOG_STREAM_LOCK: Mutex<()> = Mutex::new(());

pub fn observe_with<F>(options: ObserveOptions<'_>, factory: F) -> anyhow::Result<ObservedRun>
where
    F: FnOnce(lockin::SandboxBuilder) -> anyhow::Result<lockin::SandboxedCommand>,
{
    let stdio_backing_paths = crate::capture_stdio_backing_paths();

    if !std::path::Path::new(LOG_BIN).exists() {
        return Err(anyhow!("{LOG_BIN} not found; not a macOS host?"));
    }
    if !std::path::Path::new(SANDBOX_EXEC_BIN).exists() {
        return Err(anyhow!("{SANDBOX_EXEC_BIN} not found; not a macOS host?"));
    }

    let _log_stream_guard = LOG_STREAM_LOCK.lock().unwrap();

    let run_id = format!("lockin-run-{}", uuid::Uuid::new_v4());
    let mut log_stream = start_log_stream(&run_id)?;

    let preflight = format!("{run_id} sentinel-preflight-{}", uuid::Uuid::new_v4());
    log_stream
        .emit_and_wait_for(&preflight, SENTINEL_TIMEOUT)
        .context("log stream did not observe preflight sentinel within timeout")?;

    let builder = lockin::SandboxBuilder::new()
        .observation(crate::observation_mode(options.kind, run_id.clone()));
    let cmd = factory(builder)?;
    let status = crate::supervise(cmd, options.runtime)?;

    let postflight = format!("{run_id} sentinel-postflight-{}", uuid::Uuid::new_v4());
    log_stream
        .emit_and_wait_for(&postflight, SENTINEL_TIMEOUT)
        .context("log stream did not observe postflight sentinel within timeout")?;

    let lines = log_stream.stop_and_collect();
    let (mut events, diagnostics) = parse_log_lines(lines, &run_id);
    crate::filter_stdio_metadata_events(&mut events, &stdio_backing_paths);

    Ok(ObservedRun {
        status,
        events,
        diagnostics,
    })
}

struct LogStream {
    child: ChildGuard,
    reader_thread: Option<thread::JoinHandle<()>>,
    lines: Arc<SharedLines>,
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

    let lines = Arc::new(SharedLines::new());
    let lines_for_thread = Arc::clone(&lines);
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(log_stdout);
        for line in reader.lines().map_while(std::io::Result::ok) {
            lines_for_thread.push(line);
        }
    });

    let stream = LogStream {
        child: ChildGuard(Some(log_child)),
        reader_thread: Some(reader_thread),
        lines,
    };

    // Wait for log stream to start reading so the subsequent sentinel is not emitted too early.
    stream
        .wait_for("Filtering the log data", SENTINEL_TIMEOUT)
        .context("log stream did not print startup banner within timeout")?;

    Ok(stream)
}

impl LogStream {
    fn wait_for(&self, needle: &str, timeout: Duration) -> anyhow::Result<()> {
        self.lines.wait_for(needle, timeout)
    }

    /// Emit `needle` to the unified log on a steady cadence and wait for
    /// it to come back through the log stream. Necessary because the
    /// "Filtering the log data" banner prints before logd has actually
    /// installed our predicate; emits during that window are lost. Once
    /// the filter is live the next emit lands within milliseconds.
    fn emit_and_wait_for(&self, needle: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + timeout;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let needle_for_thread = needle.to_owned();
        let emitter = thread::spawn(move || {
            while !stop_for_thread.load(std::sync::atomic::Ordering::Relaxed) {
                if let Err(e) = emit_logger_sentinel(&needle_for_thread) {
                    eprintln!("sentinel emit failed: {e}");
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
        let result = self
            .lines
            .wait_for(needle, deadline.saturating_duration_since(Instant::now()));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = emitter.join();
        result
    }

    fn stop_and_collect(&mut self) -> Vec<String> {
        // Killing log stream EOFs its stdout, which lets the reader thread exit.
        if let Some(mut c) = self.child.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
        self.lines.drain()
    }
}

struct SharedLines {
    inner: Mutex<Vec<String>>,
    cv: Condvar,
}

impl SharedLines {
    fn new() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
            cv: Condvar::new(),
        }
    }

    fn push(&self, line: String) {
        let mut lines = self.inner.lock().unwrap();
        lines.push(line);
        self.cv.notify_all();
    }

    fn wait_for(&self, needle: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + timeout;
        let mut lines = self.inner.lock().unwrap();
        loop {
            if lines.iter().any(|line| line.contains(needle)) {
                return Ok(());
            }
            let now = Instant::now();
            if now >= deadline {
                anyhow::bail!("timed out waiting for log stream line containing {needle:?}");
            }
            let remaining = deadline.saturating_duration_since(now);
            let (guard, result) = self.cv.wait_timeout(lines, remaining).unwrap();
            lines = guard;
            if result.timed_out() && !lines.iter().any(|line| line.contains(needle)) {
                anyhow::bail!("timed out waiting for log stream line containing {needle:?}");
            }
        }
    }

    fn drain(&self) -> Vec<String> {
        std::mem::take(&mut *self.inner.lock().unwrap())
    }
}

extern "C" {
    fn lockin_observe_emit_public_log(msg: *const std::os::raw::c_char);
}

/// Emit `message` to the unified log via os_log with `%{public}s`. Going
/// through `os_log` directly (rather than `/usr/bin/logger`) is what
/// keeps the message off the privacy-redaction path: `/usr/bin/logger`'s
/// underlying syslog → unified-log bridge stores the dynamic argument
/// against a `%s` format string, which `log stream` resolves as
/// `<private>` in `composedMessage` (the field its predicate matches
/// against). On runners without `Enable-Private-Data` (e.g. GitHub
/// Actions macOS images) that redaction makes the sentinel unmatchable.
fn emit_logger_sentinel(message: &str) -> anyhow::Result<()> {
    let cstr = std::ffi::CString::new(message).context("sentinel contained NUL byte")?;
    // SAFETY: `cstr` is a valid NUL-terminated C string; the C function
    // copies the argument into the unified log and does not retain the
    // pointer past the call.
    unsafe { lockin_observe_emit_public_log(cstr.as_ptr()) };
    Ok(())
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
        // Logger sentinels prove stream readiness/drain and are not Seatbelt reports.
        if is_logger_sentinel(message, run_id) {
            continue;
        }
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

fn is_logger_sentinel(message: &str, run_id: &str) -> bool {
    message.strip_prefix(run_id).is_some_and(|suffix| {
        suffix.starts_with(" sentinel-preflight-") || suffix.starts_with(" sentinel-postflight-")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_lines_filters_logger_sentinels() {
        let run_id = "lockin-run-test";
        let line = serde_json::json!({
            "eventMessage": format!("{run_id} sentinel-preflight-1234")
        })
        .to_string();

        let (events, diagnostics) = parse_log_lines(vec![line], run_id);

        assert!(events.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn logger_sentinel_round_trips_through_log_stream() -> anyhow::Result<()> {
        let run_id = format!("lockin-run-test-{}", uuid::Uuid::new_v4());
        let mut log_stream = start_log_stream(&run_id)?;
        let sentinel = format!("{run_id} sentinel-preflight-{}", uuid::Uuid::new_v4());

        let result = log_stream.emit_and_wait_for(&sentinel, SENTINEL_TIMEOUT);
        let _ = log_stream.stop_and_collect();

        result
    }
}
