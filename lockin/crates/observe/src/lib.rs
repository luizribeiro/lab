//! Observation transport and event types for lockin.

pub mod event;
pub mod parse;
pub mod path;

#[cfg(target_os = "macos")]
pub mod darwin;
#[cfg(target_os = "linux")]
pub mod linux;

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
