//! Platform observation backends.
//!
//! A backend runs the target program under a tracer/sandbox that allows
//! every access while logging it, then returns the collected events plus
//! the child's exit status. Concrete backends are gated by target_os.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use crate::event::{InferDiagnostic, InferEvent};

#[derive(Debug, Clone)]
pub struct InferRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    /// Extra env vars to set on the child (in addition to inherited env).
    /// The backend may add its own (e.g. SYD_LOG_FD on Linux).
    pub env: Vec<(OsString, OsString)>,
}

#[derive(Debug)]
pub struct BackendReport {
    pub status: ExitStatus,
    pub events: Vec<InferEvent>,
    pub diagnostics: Vec<InferDiagnostic>,
}

/// A platform observation backend. Implementations spawn the target program
/// under a tracer/sandbox that allows every access while logging it, then
/// return the collected events plus the child's exit status.
pub trait InferBackend {
    fn run(&self, request: &InferRequest) -> anyhow::Result<BackendReport>;
}

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod darwin;
