//! Linux observation transport backed by `syd`'s tagged logging.

use std::io::{BufRead, BufReader};
use std::os::fd::{FromRawFd, OwnedFd};

use anyhow::{anyhow, Context};

use crate::parse::syd::{parse_access_line, SydParseOutcome};
use crate::{
    AccessAction, AccessEvent, DiagnosticLevel, InferDiagnostic, ObserveOptions, ObservedRun,
};

const SYD_LOG_FD: i32 = 3;

pub fn observe_with<F>(options: ObserveOptions<'_>, factory: F) -> anyhow::Result<ObservedRun>
where
    F: FnOnce(lockin::SandboxBuilder) -> anyhow::Result<lockin::SandboxedCommand>,
{
    let stdio_backing_paths = crate::capture_stdio_backing_paths();
    let run_id = format!("lockin-run-{}", uuid::Uuid::new_v4());
    let (read_fd, write_fd) = make_pipe().context("failed to create syd log pipe")?;

    let builder = lockin::SandboxBuilder::new()
        .observation(crate::observation_mode(options.kind, run_id))
        .inherit_fd_as(write_fd, SYD_LOG_FD);

    let cmd = factory(builder)?;

    // Drain in a background thread so the syd→fd3 pipe doesn't fill and block a long-running
    // child. The drain exits when both the parent and child close the write end.
    let drain = std::thread::spawn(move || drain_syd_events(read_fd));

    let status = crate::supervise(cmd, options.runtime)?;

    let (mut events, diagnostics) = drain
        .join()
        .map_err(|_| anyhow!("observe drain thread panicked"))?;
    crate::filter_stdio_metadata_events(&mut events, &stdio_backing_paths);

    Ok(ObservedRun {
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

fn drain_syd_events(read_fd: OwnedFd) -> (Vec<AccessEvent>, Vec<InferDiagnostic>) {
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
            // Inference filters to warn|deny. Allow records (which the current
            // AllowAllWithRunId backend doesn't actually emit — it uses warn — but a future
            // renderer might) are not policy-relevant to this code path.
            SydParseOutcome::Event(ae)
                if matches!(ae.action, AccessAction::Warn | AccessAction::Deny) =>
            {
                events.push(ae);
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
