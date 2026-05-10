//! Frontend supervisor module (scope §F1 + §F2 + §F3 + §F4).

pub mod shutdown;

use std::ffi::OsString;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::context::ServiceContext;
use fittings_core::error::FittingsError;
use fittings_core::message::{JsonRpcId, Request, Response};
use fittings_core::service::Service;
use fittings_server::Server;
use fittings_transport::stdio::StdioTransport;
use serde_json::Value;

use crate::broker_acl::AttachId;
use crate::bus::{Broker, PeerHandle, RegisteredFrontend};
use crate::compile::EnvPlan;
use crate::error::{BrokerError, FrontendSpawnError, InvalidFrontendPlanReason, ReaperOutcome};

const RESERVED_ENV_VARS: &[&str] = &[
    "RFL_BUS_FD",
    "RFL_PLUGIN",
    "RFL_HELPER_FD",
    "RFL_PROJECT_ROOT",
    "RFL_PRIVATE_STATE_DIR",
    "RFL_TOPIC_ID",
];

fn path_has_control_chars(path: &Path) -> bool {
    path.as_os_str()
        .as_bytes()
        .iter()
        .any(|b| *b < 0x20 || *b == 0x7f)
}

fn invalid(reason: InvalidFrontendPlanReason) -> FrontendSpawnError {
    FrontendSpawnError::InvalidPlan { reason }
}

/// Stateless factory for [`FrontendHandle`]s (scope §F1).
pub struct FrontendSupervisor {
    broker: Broker,
    config: FrontendConfig,
    extra_factory: Option<Arc<dyn FrontendExtraServiceFactory>>,
}

impl FrontendSupervisor {
    pub fn new(broker: Broker, config: FrontendConfig) -> Self {
        Self {
            broker,
            config,
            extra_factory: None,
        }
    }

    pub fn with_extra_services<F: FrontendExtraServiceFactory + 'static>(
        mut self,
        factory: F,
    ) -> Self {
        self.extra_factory = Some(Arc::new(factory));
        self
    }

    pub async fn spawn(
        &self,
        plan: &CompiledFrontend,
        paths: &FrontendPaths,
    ) -> Result<FrontendHandle, FrontendSpawnError> {
        // ---------- Phase A (cheap validation) ----------
        let attach_id = AttachId::new(plan.attach_id.clone()).map_err(|_| {
            invalid(InvalidFrontendPlanReason::AttachIdInvalid {
                attach_id: plan.attach_id.clone(),
            })
        })?;

        if path_has_control_chars(&plan.entry_absolute) {
            return Err(invalid(InvalidFrontendPlanReason::ControlCharsInPath {
                path: plan.entry_absolute.clone(),
            }));
        }
        if !plan.entry_absolute.is_absolute() {
            return Err(invalid(InvalidFrontendPlanReason::EntryNotAbsolute {
                path: plan.entry_absolute.clone(),
            }));
        }

        match std::fs::metadata(&plan.entry_absolute) {
            Ok(md) if md.is_file() && md.permissions().mode() & 0o111 != 0 => {}
            _ => {
                return Err(invalid(InvalidFrontendPlanReason::EntryNotExecutable {
                    path: plan.entry_absolute.clone(),
                }));
            }
        }

        for key in plan.env.set.keys() {
            if RESERVED_ENV_VARS.contains(&key.as_str()) {
                return Err(invalid(InvalidFrontendPlanReason::ReservedEnvName {
                    var: key.clone(),
                }));
            }
        }
        for name in &plan.env.pass {
            if RESERVED_ENV_VARS.contains(&name.as_str()) {
                return Err(invalid(InvalidFrontendPlanReason::ReservedEnvName {
                    var: name.clone(),
                }));
            }
        }

        self.broker
            .try_reserve_frontend_registration(&attach_id)
            .map_err(|e| match e {
                BrokerError::FrontendNotInAcl(a) => {
                    invalid(InvalidFrontendPlanReason::AttachIdNotInAcl { attach_id: a })
                }
                BrokerError::FrontendAlreadyRegistered(a) => {
                    invalid(InvalidFrontendPlanReason::AttachIdAlreadyRegistered { attach_id: a })
                }
                other => FrontendSpawnError::BrokerRegister { source: other },
            })?;

        // ---------- Phase B (resource allocation) ----------

        // Step 1: socketpair (CLOEXEC on both ends).
        #[cfg(target_os = "linux")]
        let cloexec_flag = nix::sys::socket::SockFlag::SOCK_CLOEXEC;
        #[cfg(not(target_os = "linux"))]
        let cloexec_flag = nix::sys::socket::SockFlag::empty();

        let (parent_fd, child_fd) = nix::sys::socket::socketpair(
            nix::sys::socket::AddressFamily::Unix,
            nix::sys::socket::SockType::Stream,
            None,
            cloexec_flag,
        )
        .map_err(|e| FrontendSpawnError::Io {
            source: std::io::Error::from_raw_os_error(e as i32),
        })?;

        // macOS post-socketpair CLOEXEC fixup.
        #[cfg(not(target_os = "linux"))]
        {
            for fd in [&parent_fd, &child_fd] {
                let raw = fd.as_raw_fd();
                let _ = nix::fcntl::fcntl(
                    raw,
                    nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
                );
            }
        }

        // Step 5 (compute first): private state dir.
        let state_dir = paths
            .project_root
            .join(".rafaello-frontend-data")
            .join(attach_id.as_str());
        std::fs::create_dir_all(&state_dir).map_err(|source| FrontendSpawnError::Io { source })?;

        // Step 2: tokio::process::Command construction.
        let mut command = tokio::process::Command::new(&plan.entry_absolute);
        command.args(&plan.argv);
        command.current_dir(&paths.project_root);

        // Step 3: env apply.
        let child_raw_fd = child_fd.as_raw_fd();
        command.env_clear();
        command.env("RFL_BUS_FD", child_raw_fd.to_string());
        command.env("RFL_PROJECT_ROOT", &paths.project_root);
        command.env("RFL_PRIVATE_STATE_DIR", &state_dir);
        for key in &plan.env.pass {
            if let Some(val) = std::env::var_os(key) {
                command.env(key, val);
            }
        }
        for (k, v) in &plan.env.set {
            command.env(k, v);
        }

        // Step 4: pre_exec — clear FD_CLOEXEC on the child socketpair end.
        unsafe {
            command.pre_exec(move || {
                let flags = nix::libc::fcntl(child_raw_fd, nix::libc::F_GETFD);
                if flags < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let new_flags = flags & !nix::libc::FD_CLOEXEC;
                if nix::libc::fcntl(child_raw_fd, nix::libc::F_SETFD, new_flags) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        // Step 6: stderr piped.
        command.stderr(std::process::Stdio::piped());
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::null());

        // Step 7: spawn.
        let mut child = command
            .spawn()
            .map_err(|source| FrontendSpawnError::Spawn { source })?;
        let cached_pid = child.id();

        // child_fd is now inherited; close our copy.
        drop(child_fd);

        // Step 8: take stderr.
        let child_stderr = child.stderr.take();

        // Step 9: readiness watch.
        let (ready_tx, ready_rx) = tokio::sync::watch::channel::<bool>(false);

        // Step 10: reaper-outcome watch.
        let (reaper_tx, reaper_rx) =
            tokio::sync::watch::channel::<Option<Arc<ReaperOutcome>>>(None);

        // Step 11: reaper + reaper-watcher tasks.
        let reaper_tx_for_reaper = reaper_tx.clone();
        let reaper_handle = tokio::spawn(async move {
            let outcome = match child.wait().await {
                Ok(s) => ReaperOutcome::Exited(s),
                Err(e) => ReaperOutcome::WaitFailed(e),
            };
            let _ = reaper_tx_for_reaper.send(Some(Arc::new(outcome)));
        });
        let reaper_tx_for_watcher = reaper_tx;
        tokio::spawn(async move {
            if reaper_handle.await.is_err() {
                let _ = reaper_tx_for_watcher.send(Some(Arc::new(ReaperOutcome::ReaperPanicked)));
            }
        });

        // Step 12: build fittings server.
        let parent_raw_fd = parent_fd.into_raw_fd();
        let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(parent_raw_fd) };
        std_stream
            .set_nonblocking(true)
            .map_err(|source| FrontendSpawnError::Io { source })?;
        let stream = tokio::net::UnixStream::from_std(std_stream)
            .map_err(|source| FrontendSpawnError::Io { source })?;
        let (reader, writer) = stream.into_split();
        let transport = StdioTransport::new(reader, writer, self.config.max_frame_bytes);

        let bus_service = FrontendBusPublishService {
            broker: self.broker.clone(),
            attach_id: attach_id.clone(),
        };
        let ready_service = FrontendReadyService { tx: ready_tx };
        let extra = self.extra_factory.as_ref().map(|f| f.build(&attach_id));
        let connection_service = FrontendConnectionService {
            bus: bus_service,
            ready: ready_service,
            extra,
        };

        let server = Server::new(connection_service, transport)
            .with_notification_capacity(self.config.notification_capacity);
        let peer = server.peer();

        // Step 13: register_frontend.
        let register_guard = self
            .broker
            .register_frontend(attach_id.clone(), peer.clone())
            .map_err(|source| FrontendSpawnError::BrokerRegister { source })?;

        // Step 14: spawn serve loop.
        let serve_handle = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        // Step 15: return handle.
        Ok(FrontendHandle {
            attach_id,
            peer,
            child_pid: cached_pid,
            serve_handle: Some(serve_handle),
            register_guard: Some(register_guard),
            child_stderr,
            ready: ready_rx,
            reaper_outcome: reaper_rx,
            config: self.config.clone(),
        })
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
    child_pid: Option<u32>,
    serve_handle: Option<tokio::task::JoinHandle<()>>,
    register_guard: Option<RegisteredFrontend>,
    child_stderr: Option<tokio::process::ChildStderr>,
    ready: tokio::sync::watch::Receiver<bool>,
    reaper_outcome: tokio::sync::watch::Receiver<Option<Arc<ReaperOutcome>>>,
    config: FrontendConfig,
}

impl FrontendHandle {
    pub async fn wait(&mut self) -> Arc<ReaperOutcome> {
        loop {
            if let Some(o) = self.reaper_outcome.borrow_and_update().clone() {
                return o;
            }
            if self.reaper_outcome.changed().await.is_err() {
                return Arc::new(ReaperOutcome::ReaperPanicked);
            }
        }
    }

    pub async fn wait_ready(&mut self) -> Result<(), FrontendReadyError> {
        loop {
            if *self.ready.borrow_and_update() {
                return Ok(());
            }
            if self.ready.changed().await.is_err() {
                return Err(FrontendReadyError::SenderDropped);
            }
        }
    }

    pub fn has_signalled_ready(&self) -> bool {
        *self.ready.borrow()
    }

    pub fn take_child_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.child_stderr.take()
    }

    pub async fn shutdown(mut self) -> ShutdownReport {
        let pid_raw = self
            .child_pid
            .take()
            .expect("FrontendHandle::shutdown called without an active child pid");
        let pid = nix::unistd::Pid::from_raw(pid_raw as i32);
        let serve = self.serve_handle.take();
        let guard = self.register_guard.take();
        let _ = self.child_stderr.take();
        let cached = self.reaper_outcome.borrow().clone();

        crate::frontend::shutdown::shutdown_with_outcome(
            cached,
            pid,
            &self.config,
            self.reaper_outcome.clone(),
            serve,
            guard,
            nix::sys::signal::kill,
            |p| nix::sys::signal::kill(p, None),
        )
        .await
    }
}

impl Drop for FrontendHandle {
    fn drop(&mut self) {
        let outcome = self.reaper_outcome.borrow().clone();
        let already_exited = matches!(outcome.as_deref(), Some(ReaperOutcome::Exited(_)));
        if !already_exited {
            if let Some(pid) = self.child_pid.take() {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
        }
        if let Some(j) = self.serve_handle.take() {
            j.abort();
        }
        drop(self.register_guard.take());
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
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "bus.publish" && req.id.is_none() {
            let _ = self
                .broker
                .handle_frontend_publish(&self.attach_id, &req.params);
            return Ok(Response {
                id: JsonRpcId::Null,
                result: Value::Null,
                metadata: Default::default(),
            });
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

/// Inbound `frontend.ready` RPC handler — flips the readiness watch
/// to `true` on first call (scope §F1, pi-4 #2).
pub struct FrontendReadyService {
    pub tx: tokio::sync::watch::Sender<bool>,
}

#[async_trait]
impl Service for FrontendReadyService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "frontend.ready" {
            if *self.tx.borrow() {
                tracing::warn!("frontend.ready called more than once");
            }
            self.tx.send_replace(true);
            let id = req.id.unwrap_or(JsonRpcId::Null);
            return Ok(Response {
                id,
                result: serde_json::json!({}),
                metadata: Default::default(),
            });
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

/// Composes additional services into the parent fittings server
/// (scope §F1, pi-2 #1).
pub trait FrontendExtraServiceFactory: Send + Sync {
    fn build(&self, attach_id: &AttachId) -> Box<dyn Service + Send + Sync>;
}

/// Routing facade: dispatches `bus.publish` to
/// [`FrontendBusPublishService`], `frontend.ready` to
/// [`FrontendReadyService`], everything else to the optional
/// extras service from [`FrontendExtraServiceFactory`].
struct FrontendConnectionService {
    bus: FrontendBusPublishService,
    ready: FrontendReadyService,
    extra: Option<Box<dyn Service + Send + Sync>>,
}

#[async_trait]
impl Service for FrontendConnectionService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "bus.publish" {
            return self.bus.call(req, ctx).await;
        }
        if req.method == "frontend.ready" {
            return self.ready.call(req, ctx).await;
        }
        if let Some(extra) = &self.extra {
            return extra.call(req, ctx).await;
        }
        Err(FittingsError::method_not_found(req.method))
    }
}
