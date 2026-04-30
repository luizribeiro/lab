//! Trace orchestration: request types, the public `trace()` entry point,
//! and the post-spawn deny-filter / canonicalize pipeline.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::{Context, Result};
use lockin_config::Config;
use lockin_observe::{AccessAction, InferDiagnostic, InferEvent};

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
    /// Optional shared tokio runtime handle. Pass when the caller is
    /// already running outpost-proxy on a runtime that must outlive
    /// the trace; if None, observation creates its own runtime.
    pub runtime: Option<tokio::runtime::Handle>,
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
    let observe_options = lockin_observe::ObserveOptions {
        kind: lockin_observe::ObservationKind::TraceDeny,
        runtime: options.runtime.as_ref(),
    };

    let raw = lockin_observe::observe_with(observe_options, |builder| {
        lockin_config::build_enforced_command(lockin_config::EnforcedCommandSpec {
            builder,
            config: &request.config,
            config_dir: request.config_dir.as_deref(),
            program: &request.program,
            args: &request.args,
            current_dir: request.current_dir.as_deref(),
            network: request.network,
            parent_env: std::env::vars_os().collect(),
            extra_env: request.env.clone(),
        })
        .context("building trace sandbox command")
    })?;

    let mut diagnostics = raw.diagnostics;
    let mut denials = Vec::new();
    for ae in raw.events {
        if ae.action != AccessAction::Deny {
            continue;
        }
        match lockin_observe::canonicalize_event(&ae.event) {
            Ok(ev) => denials.push(ev),
            Err(d) => diagnostics.push(d),
        }
    }

    let report = TraceReport {
        status: raw.status,
        denials,
        diagnostics,
    };

    if let Some(path) = &options.output {
        crate::output::write_denial_log(&report, path)?;
    }

    Ok(report)
}
