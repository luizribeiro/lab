//! Linux observation backend backed by `syd -x` (trace mode).

use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

use anyhow::{anyhow, Context, Result};

use crate::backend::{BackendReport, InferRequest};
use crate::event::InferDiagnostic;
use crate::parse::syd::{parse_line, SydParseOutcome};

const SYD_LOG_FD: i32 = 3;
// `fs` is required so the dynamic loader's `openat` syscalls aren't
// blocked by syd's default fs policy; we also set `default/fs:warn` so
// they surface as audit records (filtered downstream — see parse::syd
// classifier, which folds `fs` into Skip to avoid duplicate events).
const CATEGORIES: &str = "fs,read,stat,readdir,write,create,truncate,delete,exec";

/// Run a program under `syd -x` and collect events.
///
/// Resolves `syd` from the `LOCKIN_SYD_PATH` environment variable.
/// Returns an error if syd can't be found, can't be spawned, or fails to
/// launch the child.
pub fn run(request: &InferRequest) -> Result<BackendReport> {
    let syd_path = std::env::var_os("LOCKIN_SYD_PATH")
        .ok_or_else(|| anyhow!("LOCKIN_SYD_PATH is not set; cannot locate syd binary"))?;

    let (read_fd, write_fd) = make_pipe().context("failed to create syd log pipe")?;
    let write_raw = write_fd.as_raw_fd();

    let mut cmd = Command::new(&syd_path);
    cmd.arg("-m")
        .arg(format!("sandbox/{CATEGORIES}:on"))
        .arg("-m")
        .arg(format!("default/{CATEGORIES}:warn"))
        .env("SYD_LOG", "notice")
        .env("SYD_LOG_FD", SYD_LOG_FD.to_string())
        .env("SYD_NO_SYSLOG", "1")
        .stdin(Stdio::null());

    if let Some(dir) = &request.current_dir {
        cmd.current_dir(dir);
    }
    for (k, v) in &request.env {
        cmd.env(k, v);
    }
    cmd.arg("--").arg(&request.program).args(&request.args);

    // SAFETY: the closure runs after fork, before exec. dup2 + close are
    // async-signal-safe; we touch no Rust state that other threads share.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(write_raw, SYD_LOG_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn syd at {:?}", syd_path))?;
    let mut guard = ChildGuard(Some(child));

    // Close our copy of the write end so EOF on the read end signals
    // child exit.
    drop(write_fd);

    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    {
        let reader = BufReader::new(std::fs::File::from(read_fd));
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    diagnostics.push(InferDiagnostic {
                        level: crate::event::DiagnosticLevel::Warn,
                        message: format!("syd: log read error: {e}"),
                    });
                    break;
                }
            };
            match parse_line(&line) {
                SydParseOutcome::Event(ev) => events.push(ev),
                SydParseOutcome::Skip => {}
                SydParseOutcome::Unsupported(d) | SydParseOutcome::Malformed(d) => {
                    diagnostics.push(d);
                }
            }
        }
    }

    let mut child = guard.0.take().expect("child still owned by guard");
    let status = child.wait().context("wait on syd child failed")?;

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

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
