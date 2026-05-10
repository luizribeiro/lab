//! Frontend supervisor module (scope §F1 + §F2).
//!
//! This commit lands the public type surface only; spawn/wait/shutdown
//! bodies are placeholders.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::context::ServiceContext;
use fittings_core::error::FittingsError;
use fittings_core::message::{Request, Response};
use fittings_core::service::Service;

use crate::broker_acl::AttachId;
use crate::bus::{Broker, PeerHandle, RegisteredFrontend};
use crate::compile::EnvPlan;
use crate::error::{FrontendSpawnError, ReaperOutcome};

/// Stateless factory for [`FrontendHandle`]s (scope §F1).
pub struct FrontendSupervisor {
    #[allow(dead_code)]
    broker: Broker,
    #[allow(dead_code)]
    config: FrontendConfig,
}

impl FrontendSupervisor {
    pub fn new(broker: Broker, config: FrontendConfig) -> Self {
        Self { broker, config }
    }

    pub async fn spawn(
        &self,
        _plan: &CompiledFrontend,
        _paths: &FrontendPaths,
    ) -> Result<FrontendHandle, FrontendSpawnError> {
        unimplemented!("FrontendSupervisor::spawn lands in a later c-commit")
    }

    pub fn with_extra_services<F: FrontendExtraServiceFactory + 'static>(
        self,
        _factory: F,
    ) -> Self {
        unimplemented!("FrontendSupervisor::with_extra_services lands in a later c-commit")
    }
}

/// Spawn-time plan for a frontend (scope §F1).
pub struct CompiledFrontend {
    pub attach_id: String,
    pub entry_absolute: PathBuf,
    pub argv: Vec<OsString>,
    pub env: EnvPlan,
}

/// Filesystem paths handed to the frontend at spawn time (scope §F1).
pub struct FrontendPaths {
    pub project_root: PathBuf,
}

/// Tunable knobs for [`FrontendSupervisor`] / [`FrontendHandle`]
/// (scope §F2).
#[derive(Debug, Clone)]
pub struct FrontendConfig {
    pub shutdown_grace: Duration,
    pub shutdown_kill_grace: Duration,
    pub notification_capacity: usize,
    pub max_frame_bytes: usize,
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            shutdown_grace: Duration::from_secs(2),
            shutdown_kill_grace: Duration::from_secs(1),
            notification_capacity: 1024,
            max_frame_bytes: 1024 * 1024,
        }
    }
}

/// Lifecycle handle returned by [`FrontendSupervisor::spawn`] (scope §F1).
pub struct FrontendHandle {
    #[allow(dead_code)]
    attach_id: AttachId,
    #[allow(dead_code)]
    peer: PeerHandle,
    #[allow(dead_code)]
    child_pid: Option<u32>,
    #[allow(dead_code)]
    serve_handle: Option<tokio::task::JoinHandle<()>>,
    #[allow(dead_code)]
    register_guard: Option<RegisteredFrontend>,
    #[allow(dead_code)]
    child_stderr: Option<tokio::process::ChildStderr>,
    #[allow(dead_code)]
    ready: tokio::sync::watch::Receiver<bool>,
    #[allow(dead_code)]
    reaper_outcome: tokio::sync::watch::Receiver<Option<Arc<ReaperOutcome>>>,
    #[allow(dead_code)]
    config: FrontendConfig,
}

impl FrontendHandle {
    pub async fn wait_ready(&mut self) -> Result<(), FrontendReadyError> {
        unimplemented!("FrontendHandle::wait_ready lands in a later c-commit")
    }

    pub fn has_signalled_ready(&self) -> bool {
        unimplemented!("FrontendHandle::has_signalled_ready lands in a later c-commit")
    }

    pub async fn shutdown(self) -> ShutdownReport {
        unimplemented!("FrontendHandle::shutdown lands in a later c-commit")
    }
}

/// Result of [`FrontendHandle::shutdown`] (scope §F2).
pub struct ShutdownReport {
    pub exit_status: Option<std::process::ExitStatus>,
    pub used_sigterm: bool,
    pub used_sigkill: bool,
    pub serve_aborted: bool,
    pub elapsed: Duration,
}

/// Errors raised by [`FrontendHandle::wait_ready`] (scope §F2).
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum FrontendReadyError {
    #[error("ready-watch sender dropped before ready was signalled")]
    SenderDropped,
}

/// Errors raised while painting a TUI frame (scope §F2).
#[derive(thiserror::Error, Debug)]
pub enum PaintError {
    #[error("ratatui draw error: {0}")]
    Draw(std::io::Error),
}

/// Per-connection `bus.publish` notification handler for the
/// frontend transport (scope §F1, pi-2 #1).
pub struct FrontendBusPublishService {
    pub broker: Broker,
    pub attach_id: AttachId,
}

#[async_trait]
impl Service for FrontendBusPublishService {
    async fn call(&self, _req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        unimplemented!("FrontendBusPublishService::call lands in a later c-commit")
    }
}

/// Inbound `frontend.ready` RPC handler — flips the readiness watch
/// to `true` on first call (scope §F1, pi-4 #2).
pub struct FrontendReadyService {
    pub tx: tokio::sync::watch::Sender<bool>,
}

#[async_trait]
impl Service for FrontendReadyService {
    async fn call(&self, _req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        unimplemented!("FrontendReadyService::call lands in a later c-commit")
    }
}

/// Composes additional services into the parent fittings server
/// (scope §F1, pi-2 #1).
pub trait FrontendExtraServiceFactory: Send + Sync {
    fn build(&self, attach_id: &AttachId) -> Box<dyn Service + Send + Sync>;
}
