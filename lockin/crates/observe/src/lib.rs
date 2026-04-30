//! Observation transport and event types for lockin.

pub mod event;
pub mod parse;
pub mod path;

#[cfg(target_os = "macos")]
pub mod darwin;
#[cfg(target_os = "linux")]
pub mod linux;

use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::Context;
use lockin::ObservationMode;

pub use event::{AccessAction, AccessEvent, DiagnosticLevel, FsOp, InferDiagnostic, InferEvent};
pub use path::{canonicalize_event, canonicalize_observed};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationKind {
    /// AllowAllWithRunId: observation by allowing everything and tagging
    /// every access with the run id. Used by `lockin infer`.
    InferAllowAll,
    /// DenyTraceWithRunId: enforcement under user policy with the catch-all
    /// deny tagged so denials surface in the same stream. Used by `lockin trace`.
    TraceDeny,
}

#[derive(Debug)]
pub struct ObservedRun {
    pub status: ExitStatus,
    pub events: Vec<AccessEvent>,
    pub diagnostics: Vec<InferDiagnostic>,
}

pub struct ObserveOptions<'a> {
    pub kind: ObservationKind,
    /// Optional shared tokio runtime handle. If None, observe creates and
    /// drives its own. Callers that already manage tokio pass their handle here.
    pub runtime: Option<&'a tokio::runtime::Handle>,
}

impl<'a> ObserveOptions<'a> {
    pub fn new(kind: ObservationKind) -> Self {
        Self {
            kind,
            runtime: None,
        }
    }

    pub fn with_runtime(kind: ObservationKind, rt: &'a tokio::runtime::Handle) -> Self {
        Self {
            kind,
            runtime: Some(rt),
        }
    }
}

/// Run a sandboxed command and collect tagged access events.
///
/// `factory` receives a [`lockin::SandboxBuilder`] that already has observation mode
/// set (and on Linux fd-3 wired). The factory only needs to add policy
/// (or not, if policy-free) and call `.command(program)`.
pub fn observe_with<F>(options: ObserveOptions<'_>, factory: F) -> anyhow::Result<ObservedRun>
where
    F: FnOnce(lockin::SandboxBuilder) -> anyhow::Result<lockin::SandboxedCommand>,
{
    #[cfg(target_os = "linux")]
    {
        linux::observe_with(options, factory)
    }
    #[cfg(target_os = "macos")]
    {
        darwin::observe_with(options, factory)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (options, factory);
        anyhow::bail!("lockin observation is not implemented for this platform")
    }
}

fn capture_stdio_backing_paths() -> Vec<PathBuf> {
    (0..=2)
        .filter_map(stdio_backing_path_for_fd)
        .filter_map(|path| canonicalize_observed(&path).ok())
        .collect()
}

#[cfg(target_os = "macos")]
fn stdio_backing_path_for_fd(fd: i32) -> Option<PathBuf> {
    use std::ffi::CStr;

    let mut buf = [0 as libc::c_char; libc::PATH_MAX as usize];
    // SAFETY: `buf` is valid writable storage of PATH_MAX bytes as required by F_GETPATH.
    let rc = unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_mut_ptr()) };
    if rc == -1 {
        return None;
    }
    // SAFETY: F_GETPATH writes a NUL-terminated C string on success.
    let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
    Some(PathBuf::from(cstr.to_string_lossy().into_owned()))
}

#[cfg(target_os = "linux")]
fn stdio_backing_path_for_fd(fd: i32) -> Option<PathBuf> {
    let path = std::fs::read_link(format!("/proc/self/fd/{fd}")).ok()?;
    path.is_absolute().then_some(path)
}

fn filter_stdio_metadata_events(events: &mut Vec<AccessEvent>, stdio_paths: &[PathBuf]) {
    events.retain(|ae| !is_filtered_stdio_metadata(ae, stdio_paths));
}

fn is_filtered_stdio_metadata(ae: &AccessEvent, stdio_paths: &[PathBuf]) -> bool {
    let InferEvent::Fs {
        op: FsOp::Stat,
        path,
    } = &ae.event
    else {
        return false;
    };
    stdio_paths.iter().any(|stdio_path| stdio_path == path)
}

fn observation_mode(kind: ObservationKind, run_id: String) -> ObservationMode {
    match kind {
        ObservationKind::InferAllowAll => ObservationMode::AllowAllWithRunId(run_id),
        ObservationKind::TraceDeny => ObservationMode::DenyTraceWithRunId(run_id),
    }
}

fn supervise(
    cmd: lockin::SandboxedCommand,
    runtime: Option<&tokio::runtime::Handle>,
) -> anyhow::Result<ExitStatus> {
    if let Some(handle) = runtime {
        lockin::supervise::supervise_command(cmd, handle)
    } else {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .context("building tokio runtime for observation supervision")?;
        lockin::supervise::supervise_command(cmd, runtime.handle())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs(action: AccessAction, op: FsOp, path: &str) -> AccessEvent {
        AccessEvent {
            action,
            event: InferEvent::Fs {
                op,
                path: PathBuf::from(path),
            },
        }
    }

    #[test]
    fn filters_only_stat_events_on_stdio_backing_paths() {
        let stdio_paths = vec![PathBuf::from("/private/tmp/foo")];
        let mut events = vec![
            fs(AccessAction::Deny, FsOp::Stat, "/private/tmp/foo"),
            fs(AccessAction::Deny, FsOp::Read, "/private/tmp/foo"),
            fs(AccessAction::Deny, FsOp::Stat, "/private/tmp/bar"),
            fs(AccessAction::Warn, FsOp::Write, "/private/tmp/foo"),
            AccessEvent {
                action: AccessAction::Deny,
                event: InferEvent::Exec {
                    path: PathBuf::from("/private/tmp/foo"),
                },
            },
        ];

        filter_stdio_metadata_events(&mut events, &stdio_paths);

        assert_eq!(
            events,
            vec![
                fs(AccessAction::Deny, FsOp::Read, "/private/tmp/foo"),
                fs(AccessAction::Deny, FsOp::Stat, "/private/tmp/bar"),
                fs(AccessAction::Warn, FsOp::Write, "/private/tmp/foo"),
                AccessEvent {
                    action: AccessAction::Deny,
                    event: InferEvent::Exec {
                        path: PathBuf::from("/private/tmp/foo"),
                    },
                },
            ]
        );
    }
}
