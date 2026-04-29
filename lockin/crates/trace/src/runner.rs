//! Cross-platform trace orchestration: request types, the public
//! `trace()` entry point, and the post-spawn deny-filter / canonicalize
//! pipeline. Per-platform modules (`linux`, `darwin`) handle the actual
//! spawn + drain.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::Result;
use lockin_config::Config;
use lockin_infer::{
    canonicalize_observed, AccessAction, AccessEvent, DiagnosticLevel, InferDiagnostic, InferEvent,
};

/// Inputs to a single `trace()` run.
#[derive(Debug, Clone)]
pub struct TraceRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    /// Extra env vars to set on the child (in addition to inherited env).
    pub env: Vec<(OsString, OsString)>,
    /// The user's `lockin.toml` policy. Applied verbatim as enforcement —
    /// allow rules render normally, only the catch-all deny gets the
    /// report tag.
    pub config: Config,
    /// Directory to resolve relative paths in `config` against (typically
    /// the directory containing `lockin.toml`). Mirrors
    /// [`lockin_config::apply_config`]'s `config_dir` parameter.
    pub config_dir: Option<PathBuf>,
}

/// Knobs for the trace run. Currently no fields; reserved for the
/// commit-3 output-path argument and future extensions.
#[derive(Debug, Clone, Default)]
pub struct TraceOptions {}

/// Result of one `trace()` run.
#[derive(Debug)]
pub struct TraceReport {
    pub status: ExitStatus,
    /// Canonicalized accesses the sandbox denied (action == Deny).
    /// Other actions are filtered out at the runner boundary.
    pub denials: Vec<InferEvent>,
    /// Diagnostics from the platform backend or canonicalization.
    pub diagnostics: Vec<InferDiagnostic>,
}

/// Run `request.program` under [`lockin::ObservationMode::DenyTraceWithRunId`]
/// and return the canonicalized denial events.
pub fn trace(request: TraceRequest, options: TraceOptions) -> Result<TraceReport> {
    let _ = options;

    let (status, raw_events, mut diagnostics) = run_platform(&request)?;

    let mut denials = Vec::new();
    for ae in raw_events {
        if ae.action != AccessAction::Deny {
            continue;
        }
        match canonicalize_one(&ae.event) {
            Ok(ev) => denials.push(ev),
            Err(d) => diagnostics.push(d),
        }
    }

    Ok(TraceReport {
        status,
        denials,
        diagnostics,
    })
}

#[cfg(target_os = "linux")]
fn run_platform(
    request: &TraceRequest,
) -> Result<(ExitStatus, Vec<AccessEvent>, Vec<InferDiagnostic>)> {
    crate::linux::run(request)
}

#[cfg(target_os = "macos")]
fn run_platform(
    request: &TraceRequest,
) -> Result<(ExitStatus, Vec<AccessEvent>, Vec<InferDiagnostic>)> {
    crate::darwin::run(request)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn run_platform(
    _request: &TraceRequest,
) -> Result<(ExitStatus, Vec<AccessEvent>, Vec<InferDiagnostic>)> {
    anyhow::bail!("lockin trace is not supported on this platform")
}

/// Canonicalize observed paths (resolve `..`, symlinks, relative
/// segments) so denial events line up with the user's
/// configured policy paths. Mirrors `lockin_infer::observe`'s private
/// helper of the same name; duplicated rather than re-exported because
/// it's <30 LoC and inline visibility wins over an extra public seam.
fn canonicalize_one(ev: &InferEvent) -> std::result::Result<InferEvent, InferDiagnostic> {
    match ev {
        InferEvent::Fs { op, path } => canonicalize_observed(path)
            .map(|p| InferEvent::Fs { op: *op, path: p })
            .map_err(|e| InferDiagnostic {
                level: DiagnosticLevel::Warn,
                message: format!("dropping fs event for {}: {}", path.display(), e),
            }),
        InferEvent::Exec { path } => canonicalize_observed(path)
            .map(|p| InferEvent::Exec { path: p })
            .map_err(|e| InferDiagnostic {
                level: DiagnosticLevel::Warn,
                message: format!("dropping exec event for {}: {}", path.display(), e),
            }),
        InferEvent::Unsupported { .. } => Ok(ev.clone()),
    }
}
