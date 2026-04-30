//! Linux trace backend: `syd` in deny mode with the user's policy
//! applied as enforcement allows.
//!
//! Mirrors `crates/infer/src/backend/linux.rs`'s structure (pipe →
//! map_fd to fd 3 → supervise_command → drain JSONL events). The
//! difference is the observation mode and the fact that we apply
//! `request.config` so the user's allow rules render. Returns *all*
//! parsed events; the runner filters to `AccessAction::Deny`.

use std::io::{BufRead, BufReader};
use std::os::fd::{FromRawFd, OwnedFd};
use std::process::ExitStatus;

use anyhow::{anyhow, Context, Result};

use lockin::ObservationMode;
use lockin_infer::parse::syd::{parse_access_line, SydParseOutcome};
use lockin_infer::{AccessEvent, DiagnosticLevel, InferDiagnostic};

use crate::runner::TraceRequest;

const SYD_LOG_FD: i32 = 3;

pub(crate) fn run(
    request: &TraceRequest,
) -> Result<(ExitStatus, Vec<AccessEvent>, Vec<InferDiagnostic>)> {
    let run_id = format!("lockin-run-{}", uuid::Uuid::new_v4());

    let (read_fd, write_fd) = make_pipe().context("failed to create syd log pipe")?;

    let mut builder = lockin_config::apply_config(&request.config, request.config_dir.as_deref())
        .context("applying user lockin.toml policy")?;
    builder = builder
        .observation(ObservationMode::DenyTraceWithRunId(run_id))
        .network(request.network)
        .inherit_fd_as(write_fd, SYD_LOG_FD);

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
    cmd.stdin(std::process::Stdio::null());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("building tokio runtime for trace supervision")?;

    let drain = std::thread::spawn(move || drain_events(read_fd));

    let status = lockin::supervise::supervise_command(cmd, runtime.handle())?;

    let (events, diagnostics) = drain
        .join()
        .map_err(|_| anyhow!("trace drain thread panicked"))?;

    Ok((status, events, diagnostics))
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

fn drain_events(read_fd: OwnedFd) -> (Vec<AccessEvent>, Vec<InferDiagnostic>) {
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let reader = BufReader::new(std::fs::File::from(read_fd));
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                diagnostics.push(InferDiagnostic {
                    level: DiagnosticLevel::Warn,
                    message: format!("syd: log read error: {e}"),
                });
                break;
            }
        };
        match parse_access_line(&line) {
            SydParseOutcome::Event(ae) => events.push(ae),
            SydParseOutcome::Skip => {}
            SydParseOutcome::Unsupported(d) | SydParseOutcome::Malformed(d) => {
                diagnostics.push(d);
            }
        }
    }
    (events, diagnostics)
}
