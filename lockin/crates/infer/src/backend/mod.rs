//! Platform observation backends.
//!
//! Each backend configures a [`lockin::SandboxBuilder`] with
//! [`lockin::ObservationMode::AllowAllWithRunId`] and runs it via
//! [`lockin::supervise::supervise_command`], so dynamic-linker env
//! stripping, fd sealing, and the process-group / tcsetpgrp / signal-
//! forwarding behavior come from the unified sandbox primitive. The
//! backend's only platform-specific job is wiring the audit channel
//! the kernel-side sandbox writes to (`SYD_LOG_FD=3` pipe on Linux,
//! `os_log` ndjson stream tagged with the run's UUID on macOS) and
//! parsing those events into [`InferEvent`]s.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use lockin_observe::{InferDiagnostic, InferEvent};

#[derive(Debug, Clone)]
pub struct InferRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    /// Extra env vars to set on the child (in addition to inherited env).
    pub env: Vec<(OsString, OsString)>,
}

#[derive(Debug)]
pub struct BackendReport {
    pub status: ExitStatus,
    pub events: Vec<InferEvent>,
    pub diagnostics: Vec<InferDiagnostic>,
}

/// A platform observation backend. Implementations spawn the target
/// program under [`lockin::SandboxBuilder`] in
/// [`lockin::ObservationMode::AllowAllWithRunId`] and return the
/// collected events plus the child's exit status.
pub trait InferBackend {
    fn run(&self, request: &InferRequest) -> anyhow::Result<BackendReport>;
}

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod darwin;
