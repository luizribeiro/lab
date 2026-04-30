//! Cross-platform trace orchestration: request types, the public
//! `trace()` entry point, and the post-spawn deny-filter / canonicalize
//! pipeline. Per-platform modules (`linux`, `darwin`) handle the actual
//! spawn + drain.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::Result;
use lockin_config::Config;
use lockin_observe::{canonicalize_event, AccessAction, AccessEvent, InferDiagnostic, InferEvent};

/// Inputs to a single `trace()` run.
#[derive(Debug, Clone)]
pub struct TraceRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    /// Extra env vars to set on the child *after* `[env]` policy is
    /// applied. The CLI uses this to inject `HTTP_PROXY` etc. when
    /// `[sandbox.network] mode = "proxy"` — passed through verbatim,
    /// not blocklist-filtered.
    pub env: Vec<(OsString, OsString)>,
    /// The user's `lockin.toml` policy. Applied verbatim as enforcement —
    /// allow rules render normally, only the catch-all deny gets the
    /// report tag.
    pub config: Config,
    /// Directory to resolve relative paths in `config` against (typically
    /// the directory containing `lockin.toml`). Mirrors
    /// [`lockin_config::apply_config`]'s `config_dir` parameter.
    pub config_dir: Option<PathBuf>,
    /// Network enforcement mode. The trace runner threads this onto the
    /// builder verbatim — the CLI is responsible for resolving the
    /// `[sandbox.network]` policy and (for `Proxy`) starting the
    /// outpost-proxy and injecting `HTTP_PROXY` etc. via [`Self::env`].
    pub network: lockin::NetworkMode,
}

/// Knobs for the trace run.
#[derive(Debug, Clone, Default)]
pub struct TraceOptions {
    /// Where to write the human-readable denial log. If `None`, no
    /// file is written — callers can still inspect
    /// [`TraceReport::denials`] directly.
    pub output: Option<PathBuf>,
}

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
    let (status, raw_events, mut diagnostics) = run_platform(&request)?;

    let mut denials = Vec::new();
    for ae in raw_events {
        if ae.action != AccessAction::Deny {
            continue;
        }
        match canonicalize_event(&ae.event) {
            Ok(ev) => denials.push(ev),
            Err(d) => diagnostics.push(d),
        }
    }

    let report = TraceReport {
        status,
        denials,
        diagnostics,
    };

    if let Some(path) = &options.output {
        crate::output::write_denial_log(&report, path)?;
    }

    Ok(report)
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
