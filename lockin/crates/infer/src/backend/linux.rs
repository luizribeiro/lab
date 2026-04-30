//! Linux observation backend backed by `syd`'s warn-mode logging.
//!
//! Built on top of [`lockin::SandboxBuilder`] with
//! [`ObservationMode::AllowAllWithRunId`]: the renderer emits the
//! correct `sandbox/...:on` + `default/...:warn` rules and the
//! `SYD_LOG=notice` / `SYD_LOG_FD=3` / `SYD_NO_SYSLOG=1` env vars,
//! so this backend just wires the audit pipe to fd 3 and drains
//! events.

use std::io::{BufRead, BufReader};
use std::os::fd::{FromRawFd, OwnedFd};

use anyhow::{anyhow, Context, Result};

use lockin::{ObservationMode, SandboxBuilder};

use crate::backend::{BackendReport, InferBackend, InferRequest};
use lockin_observe::parse::syd::{parse_access_line, SydParseOutcome};
use lockin_observe::{AccessAction, InferDiagnostic, InferEvent};

const SYD_LOG_FD: i32 = 3;

/// Linux observation backend.
pub struct LinuxBackend;

impl InferBackend for LinuxBackend {
    fn run(&self, request: &InferRequest) -> Result<BackendReport> {
        run(request)
    }
}

/// Runs `request.program` under syd in observe-everything mode and
/// collects audit events. Resolves `syd` via `LOCKIN_SYD_PATH` then
/// `PATH` (handled inside [`SandboxBuilder::command`]).
pub fn run(request: &InferRequest) -> Result<BackendReport> {
    let run_id = format!("lockin-run-{}", uuid::Uuid::new_v4());

    let (read_fd, write_fd) = make_pipe().context("failed to create syd log pipe")?;

    let builder = SandboxBuilder::new()
        .observation(ObservationMode::AllowAllWithRunId(run_id))
        .inherit_fd_as(write_fd, SYD_LOG_FD);

    let mut cmd = builder
        .command(&request.program)
        .context("building infer sandbox command")?;
    cmd.args(&request.args);
    if let Some(dir) = &request.current_dir {
        cmd.current_dir(dir);
    }
    for (k, v) in &request.env {
        cmd.env(k, v);
    }
    cmd.stdin(std::process::Stdio::null());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("building tokio runtime for infer supervision")?;

    // Drain in a background thread so the syd→fd3 pipe doesn't fill
    // and block a long-running child. The drain blocks on read until
    // the parent's write end is dropped (inside `supervise_command`,
    // when it consumes the SandboxedCommand) AND the child closes
    // its fd 3 (on exit).
    let drain = std::thread::spawn(move || drain_events(read_fd));

    let status = lockin::supervise::supervise_command(cmd, runtime.handle())?;

    let (events, diagnostics) = drain
        .join()
        .map_err(|_| anyhow!("infer drain thread panicked"))?;

    Ok(BackendReport {
        status,
        events,
        diagnostics,
    })
}

fn make_pipe() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [-1i32; 2];
    // SAFETY: pipe2 writes two valid fds into the array on success.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: fds[0] and fds[1] are freshly opened by pipe2; we own them.
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    Ok((read, write))
}

fn drain_events(read_fd: OwnedFd) -> (Vec<InferEvent>, Vec<InferDiagnostic>) {
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let reader = BufReader::new(std::fs::File::from(read_fd));
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                diagnostics.push(InferDiagnostic {
                    level: lockin_observe::DiagnosticLevel::Warn,
                    message: format!("syd: log read error: {e}"),
                });
                break;
            }
        };
        match parse_access_line(&line) {
            // Inference filters to warn|deny. Allow records (which the
            // current AllowAllWithRunId backend doesn't actually emit
            // — it uses warn — but a future renderer might) are not
            // policy-relevant to this code path.
            SydParseOutcome::Event(ae)
                if matches!(ae.action, AccessAction::Warn | AccessAction::Deny) =>
            {
                events.push(ae.event);
            }
            SydParseOutcome::Event(_) => {}
            SydParseOutcome::Skip => {}
            SydParseOutcome::Unsupported(d) | SydParseOutcome::Malformed(d) => {
                diagnostics.push(d);
            }
        }
    }
    (events, diagnostics)
}
