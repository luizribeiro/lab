#![allow(clippy::result_large_err)]
//! Plugin supervisor scaffolding (scope §SP1, §SP2, §SP7).
//!
//! c14 lands the resource-ownership shape with the
//! managed-state vs. handle-observation split called out by
//! pi-1 B2: `ManagedSpawn` is the supervisor-owned record
//! (broker registration, proxy handle, serve and watcher
//! join handles) and is never shared with external
//! `SpawnHandle` clones; `SpawnObservation` is the
//! `Arc`-shared, handle-observable surface (canonical id,
//! cached pid, peer handle, cached `ReaperOutcome`).
//!
//! The Phase A/B implementation lands in c15+; c14 only
//! exposes the public types and a `spawn` stub that returns
//! [`SpawnError::SandboxBuild`] with an "unimplemented"
//! source so callers cannot accidentally take a partial
//! supervisor as ready.

use std::collections::{BTreeMap, HashSet};
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
use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::watch;

use crate::bus::{Broker, PeerHandle, RegisteredPlugin, RegisteredProvider};
use crate::compile::{CompiledPlugin, NetworkPlan};
use crate::error::{
    BrokerError, InvalidPlanReason, PathKind, ReaperOutcome, ShutdownFailure, SpawnError,
};
use crate::lock::CanonicalId;

use outpost_proxy::ProxyHandle;

pub mod core_service;
pub mod tool_catalog;

pub use core_service::CorePluginService;
pub use tool_catalog::{ToolCatalogError, ToolSchema, ToolSchemaCatalog};

/// File descriptor number the lockin sandbox maps the
/// inherited bus socket to inside the child (scope §SP7).
pub const RFL_BUS_FD_NUMBER: i32 = 3;

const RESERVED_ENV_VARS: &[&str] = &[
    "RFL_BUS_FD",
    "RFL_PLUGIN",
    "RFL_HELPER_FD",
    "RFL_PROJECT_ROOT",
    "RFL_PRIVATE_STATE_DIR",
    "RFL_TOPIC_ID",
    "RFL_PROVIDER_ID",
];

/// Supervisor-wide tunables (scope §SP1).
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub shutdown_grace: Duration,
    pub fittings_max_frame_bytes: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            shutdown_grace: Duration::from_millis(200),
            fittings_max_frame_bytes: 1 << 20,
        }
    }
}

/// Per-spawn paths the caller computes from its own layout
/// knowledge (scope §SP1). Both fields must be absolute;
/// Phase A enforces that in c15+.
#[derive(Debug, Clone)]
pub struct SpawnPaths {
    pub project_root: PathBuf,
    pub private_state_dir: PathBuf,
}

/// Result of [`PluginSupervisor::shutdown`] (scope §SP1).
#[derive(Debug, Default)]
pub struct ShutdownReport {
    pub clean: Vec<CanonicalId>,
    pub forced: Vec<CanonicalId>,
    pub failed: Vec<(CanonicalId, ShutdownFailure)>,
}

/// Handle-observable per-spawn state (scope §SP1, pi-1 B2).
///
/// `Arc`-shared with every external [`SpawnHandle`] clone;
/// the supervisor never mutates these fields after the
/// reaper task installs the cached `ReaperOutcome`.
struct SpawnObservation {
    canonical: CanonicalId,
    topic_id: String,
    cached_pid: Option<u32>,
    peer: PeerHandle,
    outcome: watch::Receiver<Option<Arc<ReaperOutcome>>>,
}

/// Cloneable handle to a spawned plugin (scope §SP1).
///
/// Holds only the `Arc<SpawnObservation>` — dropping every
/// `SpawnHandle` clone does not affect child lifetime; the
/// supervisor owns the child via `ManagedSpawn` and kills
/// it on `shutdown` / `Drop` regardless.
#[derive(Clone)]
pub struct SpawnHandle(Arc<SpawnObservation>);

impl SpawnHandle {
    pub fn canonical(&self) -> &CanonicalId {
        &self.0.canonical
    }

    pub fn topic_id(&self) -> &str {
        &self.0.topic_id
    }

    pub fn child_pid(&self) -> Option<u32> {
        if self.0.outcome.borrow().is_some() {
            None
        } else {
            self.0.cached_pid
        }
    }

    pub fn peer(&self) -> &PeerHandle {
        &self.0.peer
    }

    pub async fn wait(&self) -> Arc<ReaperOutcome> {
        let mut rx = self.0.outcome.clone();
        loop {
            if let Some(outcome) = rx.borrow_and_update().clone() {
                return outcome;
            }
            if rx.changed().await.is_err() {
                // Sender dropped without sending — only possible
                // if the reaper task was cancelled before ever
                // observing exit. Surface as ReaperPanicked so
                // callers always see a terminal outcome.
                return Arc::new(ReaperOutcome::ReaperPanicked);
            }
        }
    }

    pub fn try_wait(&self) -> Option<Arc<ReaperOutcome>> {
        self.0.outcome.borrow().clone()
    }
}

/// Supervisor-owned per-spawn resources (scope §SP1, pi-1 B2).
///
/// Pi-2 B2: only `watcher_join` is stored, never the reaper
/// `JoinHandle`. The watcher task consumes the reaper join
/// via `.await` and translates a `JoinError` into
/// `ReaperOutcome::ReaperPanicked`.
#[allow(dead_code)] // c15+ wires teardown through these fields.
struct ManagedSpawn {
    observation: Arc<SpawnObservation>,
    registered: Option<SpawnRegistration>,
    proxy: Option<ProxyHandle>,
    serve_join: Option<tokio::task::JoinHandle<()>>,
    watcher_join: Option<tokio::task::JoinHandle<()>>,
}

/// Discriminates plugin vs. provider broker registration so the
/// supervisor's RAII drop releases the right registry slot
/// (scope §M2.2). Holds the broker registration guard returned
/// by either `Broker::register_plugin` or
/// `Broker::register_provider`.
#[allow(dead_code)] // Held only for RAII drop; never read.
enum SpawnRegistration {
    Plugin(RegisteredPlugin),
    Provider(RegisteredProvider),
}

/// Test-only counters (scope §SP2).
///
/// All wired so callers can read real values; later commits
/// increment them as the spawn pipeline lights up.
#[cfg(any(test, feature = "test-fixture"))]
#[derive(Debug)]
pub struct TestHooks {
    pub outpost_starts: std::sync::atomic::AtomicUsize,
    pub socketpair_creates: std::sync::atomic::AtomicUsize,
    pub child_spawns: std::sync::atomic::AtomicUsize,
    /// Last loopback port bound by `outpost_proxy::start` during a
    /// spawn under proxy mode. `0` if no proxy was started. Lets
    /// tests poll for asynchronous `ProxyHandle` unbind without
    /// reaching into private supervisor state.
    pub last_proxy_port: std::sync::atomic::AtomicU16,
    /// Pid stored by the supervisor's post-spawn unwind path when a
    /// failure after `cmd.spawn()` triggers SIGKILL + reap. `-1`
    /// means "no reap has fired yet". Stored as `AtomicI64` so a
    /// `u32` pid plus the "none" sentinel both round-trip.
    pub last_reaped_pid: std::sync::atomic::AtomicI64,
    /// One-shot fault injectors (scope §H6.1). When set to `true`
    /// the matching point in `PluginSupervisor::spawn` swaps the
    /// flag back to `false`, sets the matching `*_consumed` flag,
    /// and returns `SpawnError::SandboxBuild`.
    pub inject_pre_spawn_fault: std::sync::atomic::AtomicBool,
    pub inject_post_spawn_pre_register_fault: std::sync::atomic::AtomicBool,
    pub inject_post_register_fault: std::sync::atomic::AtomicBool,
    pre_spawn_fault_consumed_flag: std::sync::atomic::AtomicBool,
    post_spawn_pre_register_fault_consumed_flag: std::sync::atomic::AtomicBool,
    post_register_fault_consumed_flag: std::sync::atomic::AtomicBool,
    /// c38: monotonic-clock nanos at the first `spawn` call. Lets
    /// integration tests assert that the [`crate::gate::ConfirmationGate`]
    /// subscribed BEFORE any plugin was spawned. `0` until the first
    /// spawn fires.
    first_spawn_instant_nanos: std::sync::atomic::AtomicU64,
}

#[cfg(any(test, feature = "test-fixture"))]
impl Default for TestHooks {
    fn default() -> Self {
        Self {
            outpost_starts: std::sync::atomic::AtomicUsize::new(0),
            socketpair_creates: std::sync::atomic::AtomicUsize::new(0),
            child_spawns: std::sync::atomic::AtomicUsize::new(0),
            last_proxy_port: std::sync::atomic::AtomicU16::new(0),
            last_reaped_pid: std::sync::atomic::AtomicI64::new(-1),
            inject_pre_spawn_fault: std::sync::atomic::AtomicBool::new(false),
            inject_post_spawn_pre_register_fault: std::sync::atomic::AtomicBool::new(false),
            inject_post_register_fault: std::sync::atomic::AtomicBool::new(false),
            pre_spawn_fault_consumed_flag: std::sync::atomic::AtomicBool::new(false),
            post_spawn_pre_register_fault_consumed_flag: std::sync::atomic::AtomicBool::new(false),
            post_register_fault_consumed_flag: std::sync::atomic::AtomicBool::new(false),
            first_spawn_instant_nanos: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[cfg(any(test, feature = "test-fixture"))]
impl TestHooks {
    /// Convert the `AtomicI64` reap sentinel into an `Option<u32>`.
    pub fn last_reaped_pid(&self) -> Option<u32> {
        let raw = self
            .last_reaped_pid
            .load(std::sync::atomic::Ordering::SeqCst);
        if raw < 0 {
            None
        } else {
            Some(raw as u32)
        }
    }

    pub fn pre_spawn_fault_consumed(&self) -> bool {
        self.pre_spawn_fault_consumed_flag
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn post_spawn_pre_register_fault_consumed(&self) -> bool {
        self.post_spawn_pre_register_fault_consumed_flag
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn post_register_fault_consumed(&self) -> bool {
        self.post_register_fault_consumed_flag
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// c38: monotonic nanos at which the first `PluginSupervisor::spawn`
    /// fired in this supervisor. `None` until any spawn starts. Used by
    /// `rfl_chat_constructs_gate_before_provider_spawn` to assert
    /// `gate.subscribed_at < first_spawn_at`.
    pub fn first_spawn_instant_nanos(&self) -> Option<u64> {
        let raw = self
            .first_spawn_instant_nanos
            .load(std::sync::atomic::Ordering::SeqCst);
        if raw == 0 {
            None
        } else {
            Some(raw)
        }
    }

    pub(crate) fn record_first_spawn_now(&self) {
        let now_nanos = monotonic_nanos();
        let _ = self.first_spawn_instant_nanos.compare_exchange(
            0,
            now_nanos,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

#[cfg(any(test, feature = "test-fixture"))]
pub fn monotonic_nanos() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    Instant::now()
        .saturating_duration_since(*epoch)
        .as_nanos()
        .min(u64::MAX as u128) as u64
        + 1
}

/// Test-only factory for additional `core.fixture.*` services
/// composed alongside the production `bus.publish` handler
/// (scope §SP2).
#[cfg(any(test, feature = "test-fixture"))]
pub type ExtraServiceFactory = Arc<
    dyn Fn(CanonicalId) -> Box<dyn fittings_core::service::Service + Send + Sync> + Send + Sync,
>;

pub struct PluginSupervisor {
    broker: Broker,
    #[allow(dead_code)]
    config: SupervisorConfig,
    in_flight: Arc<Mutex<HashSet<CanonicalId>>>,
    #[allow(dead_code)]
    managed: Mutex<BTreeMap<CanonicalId, ManagedSpawn>>,
    lazy_candidates: Mutex<BTreeMap<CanonicalId, LazyCandidate>>,
    #[allow(dead_code)]
    tool_to_canonical: Mutex<BTreeMap<String, CanonicalId>>,
    tool_catalog: Arc<ToolSchemaCatalog>,
    #[cfg(any(test, feature = "test-fixture"))]
    test_hooks: Arc<TestHooks>,
    #[cfg(any(test, feature = "test-fixture"))]
    #[allow(dead_code)]
    extra_service_factory: Option<ExtraServiceFactory>,
}

struct LazyCandidate {
    plan: CompiledPlugin,
    paths: SpawnPaths,
    #[allow(dead_code)]
    triggers: Vec<String>,
}

impl PluginSupervisor {
    pub fn new(
        broker: Broker,
        config: SupervisorConfig,
        tool_catalog: Arc<ToolSchemaCatalog>,
    ) -> Self {
        Self {
            broker,
            config,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            managed: Mutex::new(BTreeMap::new()),
            lazy_candidates: Mutex::new(BTreeMap::new()),
            tool_to_canonical: Mutex::new(BTreeMap::new()),
            tool_catalog,
            #[cfg(any(test, feature = "test-fixture"))]
            test_hooks: Arc::new(TestHooks::default()),
            #[cfg(any(test, feature = "test-fixture"))]
            extra_service_factory: None,
        }
    }

    pub fn register_lazy(
        &self,
        canonical: CanonicalId,
        plan: CompiledPlugin,
        paths: SpawnPaths,
        triggers: Vec<String>,
    ) {
        let candidate = LazyCandidate {
            plan,
            paths,
            triggers: triggers.clone(),
        };
        self.lazy_candidates
            .lock()
            .insert(canonical.clone(), candidate);
        let mut t2c = self.tool_to_canonical.lock();
        for tool in triggers {
            t2c.insert(tool, canonical.clone());
        }
    }

    /// Ensure the plugin for `canonical` is spawned.
    ///
    /// - `Ok(false)`: already in `managed` (idempotent re-dispatch
    ///   — pi-5 B-4) OR no lazy candidate registered for this
    ///   canonical (eager-tool no-op — pi-5 B-3). Caller proceeds
    ///   with normal dispatch.
    /// - `Ok(true)`: was a lazy candidate, just spawned now.
    /// - `Err(SpawnError)`: candidate exists but `spawn` failed.
    ///   Candidate is restored to `lazy_candidates` so a retry can
    ///   find it.
    pub async fn ensure_spawned(&self, canonical: &CanonicalId) -> Result<bool, SpawnError> {
        if self.managed.lock().contains_key(canonical) {
            return Ok(false);
        }
        let candidate = {
            let mut lazy = self.lazy_candidates.lock();
            lazy.remove(canonical)
        };
        let Some(candidate) = candidate else {
            return Ok(false);
        };
        record_spawn_event(canonical, "spawn_on_demand");
        match self.spawn(&candidate.plan, &candidate.paths).await {
            Ok(_handle) => Ok(true),
            Err(e) => {
                self.lazy_candidates
                    .lock()
                    .insert(canonical.clone(), candidate);
                Err(e)
            }
        }
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn with_extra_service(
        broker: Broker,
        config: SupervisorConfig,
        factory: ExtraServiceFactory,
    ) -> Self {
        let mut s = Self::new(broker, config, ToolSchemaCatalog::empty_for_tests());
        s.extra_service_factory = Some(factory);
        s
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub async fn dispatch_for_tests(
        &self,
        canonical: CanonicalId,
        req: Request,
        ctx: ServiceContext,
    ) -> Result<Response, FittingsError> {
        self.build_connection_service(canonical)
            .call(req, ctx)
            .await
    }

    fn is_provider(&self, canonical: &CanonicalId) -> bool {
        self.broker
            .plugin_acl(canonical)
            .and_then(|a| a.provider_id)
            .is_some()
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn test_hooks(&self) -> Arc<TestHooks> {
        Arc::clone(&self.test_hooks)
    }

    /// Test-only inspection of the `in_flight` reservation set
    /// (scope §SP4 step 1a). Lets integration tests assert that
    /// post-failure unwind drained the canonical without exposing
    /// the field publicly.
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn is_in_flight(&self, canonical: &CanonicalId) -> bool {
        self.in_flight.lock().contains(canonical)
    }

    pub async fn spawn(
        &self,
        plan: &CompiledPlugin,
        paths: &SpawnPaths,
    ) -> Result<SpawnHandle, SpawnError> {
        #[cfg(any(test, feature = "test-fixture"))]
        self.test_hooks.record_first_spawn_now();
        let in_flight_guard = InFlightGuard::acquire(Arc::clone(&self.in_flight), &plan.canonical)?;

        self.broker
            .try_reserve_registration(&plan.canonical)
            .map_err(|e| match e {
                BrokerError::NotInAcl(c) => SpawnError::NotInAcl(c),
                BrokerError::AlreadyRegistered(c) => SpawnError::AlreadyRegistered(c),
                other => SpawnError::SandboxBuild {
                    canonical: plan.canonical.clone(),
                    source: anyhow::anyhow!(other),
                },
            })?;

        let acl_provider_id = match self.broker.plugin_acl(&plan.canonical) {
            Some(acl) => {
                if acl.topic_id != plan.topic_id {
                    return Err(SpawnError::InvalidPlan {
                        canonical: plan.canonical.clone(),
                        reason: InvalidPlanReason::TopicIdMismatch {
                            expected: acl.topic_id,
                            got: plan.topic_id.clone(),
                        },
                    });
                }
                acl.provider_id
            }
            None => return Err(SpawnError::NotInAcl(plan.canonical.clone())),
        };

        validate_path(
            &plan.entry_absolute,
            PathKind::EntryAbsolute,
            &plan.canonical,
        )?;
        validate_path(&paths.project_root, PathKind::ProjectRoot, &plan.canonical)?;
        validate_path(
            &paths.private_state_dir,
            PathKind::PrivateStateDir,
            &plan.canonical,
        )?;
        for p in &plan.filesystem.read_paths {
            validate_path(p, PathKind::ReadPath, &plan.canonical)?;
        }
        for p in &plan.filesystem.read_dirs {
            validate_path(p, PathKind::ReadDir, &plan.canonical)?;
        }
        for p in &plan.filesystem.write_paths {
            validate_path(p, PathKind::WritePath, &plan.canonical)?;
        }
        for p in &plan.filesystem.write_dirs {
            validate_path(p, PathKind::WriteDir, &plan.canonical)?;
        }
        for p in &plan.filesystem.exec_paths {
            validate_path(p, PathKind::ExecPath, &plan.canonical)?;
        }
        for p in &plan.filesystem.exec_dirs {
            validate_path(p, PathKind::ExecDir, &plan.canonical)?;
        }

        for key in plan.env.set.keys() {
            if is_reserved_env(key) {
                return Err(SpawnError::ReservedEnvInPlan {
                    canonical: plan.canonical.clone(),
                    var: key.clone(),
                });
            }
        }
        for name in &plan.env.pass {
            if is_reserved_env(name) {
                return Err(SpawnError::ReservedEnvInPlan {
                    canonical: plan.canonical.clone(),
                    var: name.clone(),
                });
            }
        }

        if let NetworkPlan::Proxy { allow_hosts } = &plan.network {
            if let Err(source) =
                outpost::NetworkPolicy::from_allowed_hosts(allow_hosts.iter().map(String::as_str))
            {
                return Err(SpawnError::InvalidPlan {
                    canonical: plan.canonical.clone(),
                    reason: InvalidPlanReason::NetworkAllowHostsInvalid { source },
                });
            }
        }

        match std::fs::metadata(&plan.entry_absolute) {
            Ok(md) if md.is_file() && md.permissions().mode() & 0o111 != 0 => {}
            _ => {
                return Err(SpawnError::EntryNotExecutable {
                    canonical: plan.canonical.clone(),
                    path: plan.entry_absolute.clone(),
                });
            }
        }

        // Per-platform CLOEXEC handling: nix's `SockFlag::SOCK_CLOEXEC`
        // is Linux-only; macOS needs `fcntl(F_SETFD, FD_CLOEXEC)`
        // after socketpair. m2 retrospective §5.7 (CI follow-up).
        #[cfg(target_os = "linux")]
        let cloexec_flag = nix::sys::socket::SockFlag::SOCK_CLOEXEC;
        #[cfg(not(target_os = "linux"))]
        let cloexec_flag = nix::sys::socket::SockFlag::empty();

        let (core_fd, child_fd) = nix::sys::socket::socketpair(
            nix::sys::socket::AddressFamily::Unix,
            nix::sys::socket::SockType::Stream,
            None,
            cloexec_flag,
        )
        .map_err(|source| SpawnError::Socketpair {
            canonical: plan.canonical.clone(),
            source,
        })?;

        // macOS post-socketpair CLOEXEC fixup (Linux already set the flag
        // atomically via SOCK_CLOEXEC). Uses nix's fcntl wrapper.
        #[cfg(not(target_os = "linux"))]
        {
            use std::os::fd::AsRawFd;
            for fd in [&core_fd, &child_fd] {
                let raw = fd.as_raw_fd();
                let _ = nix::fcntl::fcntl(
                    raw,
                    nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
                );
            }
        }
        #[cfg(any(test, feature = "test-fixture"))]
        self.test_hooks
            .socketpair_creates
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (proxy, proxy_port): (Option<ProxyHandle>, u16) = match &plan.network {
            NetworkPlan::Proxy { allow_hosts } => {
                let policy = outpost::NetworkPolicy::from_allowed_hosts(
                    allow_hosts.iter().map(String::as_str),
                )
                .map_err(|source| SpawnError::InvalidPlan {
                    canonical: plan.canonical.clone(),
                    reason: InvalidPlanReason::NetworkAllowHostsInvalid { source },
                })?;
                let handle = outpost_proxy::start(policy).await.map_err(|source| {
                    SpawnError::ProxyStart {
                        canonical: plan.canonical.clone(),
                        source,
                    }
                })?;
                #[cfg(any(test, feature = "test-fixture"))]
                self.test_hooks
                    .outpost_starts
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let port = handle.listen_addr().port();
                #[cfg(any(test, feature = "test-fixture"))]
                self.test_hooks
                    .last_proxy_port
                    .store(port, std::sync::atomic::Ordering::SeqCst);
                (Some(handle), port)
            }
            _ => (None, 0),
        };

        let mut builder = lockin::Sandbox::builder();
        for p in &plan.filesystem.read_paths {
            builder = builder.read_path(p);
        }
        for p in &plan.filesystem.read_dirs {
            builder = builder.read_dir(p);
        }
        for p in &plan.filesystem.write_paths {
            builder = builder.write_path(p);
        }
        for p in &plan.filesystem.write_dirs {
            builder = builder.write_dir(p);
        }
        for p in &plan.filesystem.exec_paths {
            builder = builder.exec_path(p);
        }
        for p in &plan.filesystem.exec_dirs {
            builder = builder.exec_dir(p);
        }
        builder = match &plan.network {
            NetworkPlan::Deny => builder.network_deny(),
            NetworkPlan::AllowAll => builder.network_allow_all(),
            NetworkPlan::Proxy { .. } => builder.network_proxy(proxy_port),
        };
        builder = builder
            .max_cpu_time(plan.limits.max_cpu_time)
            .max_open_files(plan.limits.max_open_files)
            .disable_core_dumps();
        if let Some(n) = plan.limits.max_address_space {
            builder = builder.max_address_space(n);
        }
        if let Some(n) = plan.limits.max_processes {
            builder = builder.max_processes(n);
        }
        builder = builder.inherit_fd_as(child_fd, RFL_BUS_FD_NUMBER as std::os::fd::RawFd);

        if let Err(source) = std::fs::create_dir_all(&paths.private_state_dir) {
            return Err(SpawnError::PrivateStateDirCreate {
                canonical: plan.canonical.clone(),
                path: paths.private_state_dir.clone(),
                source,
            });
        }

        let mut cmd = builder
            .tokio_command(&plan.entry_absolute)
            .map_err(|source| SpawnError::SandboxBuild {
                canonical: plan.canonical.clone(),
                source,
            })?;

        // NOTE per scope §SP4: lockin's env_clear removes TMPDIR/TMP/TEMP; lockin's
        // SandboxedCommand does not expose private_tmp pre-spawn (pi-2 §5), so m2 cannot
        // re-inject those vars. Plugins use RFL_PRIVATE_STATE_DIR for scratch (NOT a
        // plugin ABI guarantee per pi runtime-extensibility — m2 retrospective records).
        cmd.env_clear();
        for key in &plan.env.pass {
            if let Some(val) = std::env::var_os(key) {
                cmd.env(key, val);
            }
        }
        for (k, v) in &plan.env.set {
            cmd.env(k, v);
        }
        if let NetworkPlan::Proxy { .. } = &plan.network {
            let proxy_url = format!("http://127.0.0.1:{}", proxy_port);
            for key in [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
                "http_proxy",
                "https_proxy",
                "all_proxy",
            ] {
                cmd.env(key, &proxy_url);
            }
            for key in ["NO_PROXY", "no_proxy"] {
                cmd.env(key, "");
            }
        }
        cmd.env("RFL_BUS_FD", RFL_BUS_FD_NUMBER.to_string());
        cmd.env("RFL_PLUGIN", plan.canonical.to_string());
        cmd.env("RFL_PROJECT_ROOT", &paths.project_root);
        cmd.env("RFL_PRIVATE_STATE_DIR", &paths.private_state_dir);
        cmd.env("RFL_TOPIC_ID", &plan.topic_id);
        if let Some(provider_id) = acl_provider_id.as_deref() {
            cmd.env("RFL_PROVIDER_ID", provider_id);
        }
        cmd.current_dir(&paths.project_root);

        #[cfg(any(test, feature = "test-fixture"))]
        if self
            .test_hooks
            .inject_pre_spawn_fault
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.test_hooks
                .pre_spawn_fault_consumed_flag
                .store(true, std::sync::atomic::Ordering::SeqCst);
            drop(proxy);
            drop(core_fd);
            return Err(SpawnError::SandboxBuild {
                canonical: plan.canonical.clone(),
                source: anyhow::anyhow!("test-injected pre-spawn fault"),
            });
        }

        // Step 13: spawn the child. After this point every error
        // path must SIGKILL + `child.wait().await` to reap before
        // returning, per the post-spawn unwind contract (scope §SP4
        // Phase B).
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(source) => {
                drop(proxy);
                drop(core_fd);
                return Err(SpawnError::Spawn {
                    canonical: plan.canonical.clone(),
                    source,
                });
            }
        };
        #[cfg(any(test, feature = "test-fixture"))]
        self.test_hooks
            .child_spawns
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let cached_pid = child.id();

        // Step 14: convert core_fd → tokio UnixStream.
        let std_stream = std::os::unix::net::UnixStream::from(core_fd);
        if let Err(source) = std_stream.set_nonblocking(true) {
            self.kill_and_reap(&mut child, cached_pid).await;
            drop(proxy);
            return Err(SpawnError::TransportSetup {
                canonical: plan.canonical.clone(),
                source,
            });
        }
        let stream = match tokio::net::UnixStream::from_std(std_stream) {
            Ok(s) => s,
            Err(source) => {
                self.kill_and_reap(&mut child, cached_pid).await;
                drop(proxy);
                return Err(SpawnError::TransportSetup {
                    canonical: plan.canonical.clone(),
                    source,
                });
            }
        };

        // Step 15: split + build StdioTransport.
        let (reader, writer) = stream.into_split();
        let transport = fittings_transport::stdio::StdioTransport::new(
            reader,
            writer,
            self.config.fittings_max_frame_bytes,
        );

        // Step 16: per-connection service.
        let service = self.build_connection_service(plan.canonical.clone());

        // Step 17: build server, capture peer, register with broker.
        let server = Server::new(service, transport);
        let peer = server.peer();

        #[cfg(any(test, feature = "test-fixture"))]
        if self
            .test_hooks
            .inject_post_spawn_pre_register_fault
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.test_hooks
                .post_spawn_pre_register_fault_consumed_flag
                .store(true, std::sync::atomic::Ordering::SeqCst);
            self.kill_and_reap(&mut child, cached_pid).await;
            drop(server);
            drop(proxy);
            return Err(SpawnError::SandboxBuild {
                canonical: plan.canonical.clone(),
                source: anyhow::anyhow!("test-injected post-spawn-pre-register fault"),
            });
        }

        let registered = match acl_provider_id.is_some() {
            true => match self
                .broker
                .register_provider(plan.canonical.clone(), peer.clone())
            {
                Ok(r) => SpawnRegistration::Provider(r),
                Err(BrokerError::ProviderNotInAcl(c)) => {
                    self.kill_and_reap(&mut child, cached_pid).await;
                    drop(server);
                    drop(proxy);
                    return Err(SpawnError::NotInAcl(c));
                }
                Err(BrokerError::ProviderAlreadyRegistered(c)) => {
                    self.kill_and_reap(&mut child, cached_pid).await;
                    drop(server);
                    drop(proxy);
                    return Err(SpawnError::AlreadyRegistered(c));
                }
                Err(other) => {
                    self.kill_and_reap(&mut child, cached_pid).await;
                    drop(server);
                    drop(proxy);
                    return Err(SpawnError::SandboxBuild {
                        canonical: plan.canonical.clone(),
                        source: anyhow::anyhow!(other),
                    });
                }
            },
            false => match self
                .broker
                .register_plugin(plan.canonical.clone(), peer.clone())
            {
                Ok(r) => SpawnRegistration::Plugin(r),
                Err(BrokerError::NotInAcl(c)) => {
                    self.kill_and_reap(&mut child, cached_pid).await;
                    drop(server);
                    drop(proxy);
                    return Err(SpawnError::NotInAcl(c));
                }
                Err(BrokerError::AlreadyRegistered(c)) => {
                    self.kill_and_reap(&mut child, cached_pid).await;
                    drop(server);
                    drop(proxy);
                    return Err(SpawnError::AlreadyRegistered(c));
                }
                Err(other) => {
                    self.kill_and_reap(&mut child, cached_pid).await;
                    drop(server);
                    drop(proxy);
                    return Err(SpawnError::SandboxBuild {
                        canonical: plan.canonical.clone(),
                        source: anyhow::anyhow!(other),
                    });
                }
            },
        };

        // Registration is now the source of truth — drop the
        // in-flight reservation immediately (scope §SP4 step 17,
        // pi-6 non-blocking #5).
        drop(in_flight_guard);

        // Step 18: reaper task owns the child and publishes the
        // terminal `ReaperOutcome` to the watch channel; the watcher
        // task consumes the reaper `JoinHandle` (pi-2 B2 — single
        // owner) and translates a panic into `ReaperPanicked`.
        let (watch_tx, watch_rx) = watch::channel::<Option<Arc<ReaperOutcome>>>(None);
        let watch_tx_clone = watch_tx.clone();
        #[cfg(any(test, feature = "test-fixture"))]
        let reap_hooks = Arc::clone(&self.test_hooks);
        let reaper_handle = tokio::spawn(async move {
            let outcome = match child.wait().await {
                Ok(s) => ReaperOutcome::Exited(s),
                Err(e) => ReaperOutcome::WaitFailed(e),
            };
            #[cfg(any(test, feature = "test-fixture"))]
            if let Some(pid) = cached_pid {
                reap_hooks
                    .last_reaped_pid
                    .store(pid as i64, std::sync::atomic::Ordering::SeqCst);
            }
            let _ = watch_tx.send(Some(Arc::new(outcome)));
        });
        let watcher_join = tokio::spawn(async move {
            if let Err(_join_err) = reaper_handle.await {
                let _ = watch_tx_clone.send(Some(Arc::new(ReaperOutcome::ReaperPanicked)));
            }
        });

        #[cfg(any(test, feature = "test-fixture"))]
        if self
            .test_hooks
            .inject_post_register_fault
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.test_hooks
                .post_register_fault_consumed_flag
                .store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(pid) = cached_pid {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
            watcher_join.abort();
            drop(registered);
            drop(server);
            drop(proxy);
            return Err(SpawnError::SandboxBuild {
                canonical: plan.canonical.clone(),
                source: anyhow::anyhow!("test-injected post-register fault"),
            });
        }

        // Step 19: serve loop drives the per-connection fittings server.
        let serve_join = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        // Step 20: build observation + managed record, return handle.
        let observation = Arc::new(SpawnObservation {
            canonical: plan.canonical.clone(),
            topic_id: plan.topic_id.clone(),
            cached_pid,
            peer,
            outcome: watch_rx,
        });
        let managed = ManagedSpawn {
            observation: Arc::clone(&observation),
            registered: Some(registered),
            proxy,
            serve_join: Some(serve_join),
            watcher_join: Some(watcher_join),
        };
        self.managed.lock().insert(plan.canonical.clone(), managed);
        Ok(SpawnHandle(observation))
    }

    fn build_connection_service(&self, canonical: CanonicalId) -> SupervisorConnectionService {
        let bus = BusPublishService {
            broker: self.broker.clone(),
            canonical: canonical.clone(),
        };
        let core = if self.is_provider(&canonical) {
            Some(CorePluginService {
                catalog: self.tool_catalog.clone(),
            })
        } else {
            None
        };
        #[cfg(any(test, feature = "test-fixture"))]
        let extra = self
            .extra_service_factory
            .as_ref()
            .map(|factory| factory(canonical));
        #[cfg(any(test, feature = "test-fixture"))]
        return SupervisorConnectionService { bus, core, extra };
        #[cfg(not(any(test, feature = "test-fixture")))]
        SupervisorConnectionService { bus, core }
    }

    async fn kill_and_reap(
        &self,
        child: &mut lockin::tokio::SandboxedChild,
        cached_pid: Option<u32>,
    ) {
        if let Some(pid) = cached_pid {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
        let _ = child.wait().await;
        #[cfg(any(test, feature = "test-fixture"))]
        if let Some(pid) = cached_pid {
            self.test_hooks
                .last_reaped_pid
                .store(pid as i64, std::sync::atomic::Ordering::SeqCst);
        }
    }

    pub async fn shutdown(&self) -> ShutdownReport {
        let drained: Vec<(CanonicalId, ManagedSpawn)> = {
            let mut guard = self.managed.lock();
            std::mem::take(&mut *guard).into_iter().collect()
        };

        let mut report = ShutdownReport::default();
        let grace = self.config.shutdown_grace;

        for (canonical, mut managed) in drained {
            drop(managed.registered.take());

            let mut rx = managed.observation.outcome.clone();
            let cached_pid = managed.observation.cached_pid;
            let mut forced = false;

            if rx.borrow_and_update().is_none() {
                if let Some(pid) = cached_pid {
                    let pid_t = nix::unistd::Pid::from_raw(pid as i32);
                    match nix::sys::signal::kill(pid_t, nix::sys::signal::Signal::SIGTERM) {
                        Ok(()) => {}
                        Err(nix::errno::Errno::ESRCH) => {}
                        Err(e) => {
                            drop(managed.proxy.take());
                            if let Some(j) = managed.serve_join.take() {
                                j.abort();
                            }
                            report
                                .failed
                                .push((canonical, ShutdownFailure::SignalSendFailed(e)));
                            continue;
                        }
                    }

                    let graceful = tokio::time::timeout(grace, async {
                        loop {
                            if rx.borrow_and_update().is_some() {
                                return;
                            }
                            if rx.changed().await.is_err() {
                                return;
                            }
                        }
                    })
                    .await;

                    if graceful.is_err() {
                        forced = true;
                        let _ = nix::sys::signal::kill(pid_t, nix::sys::signal::Signal::SIGKILL);
                        loop {
                            if rx.borrow_and_update().is_some() {
                                break;
                            }
                            if rx.changed().await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }

            drop(managed.proxy.take());
            if let Some(j) = managed.serve_join.take() {
                j.abort();
            }

            let outcome = rx.borrow().clone();
            match outcome.as_deref() {
                Some(ReaperOutcome::Exited(_)) => {
                    if forced {
                        report.forced.push(canonical);
                    } else {
                        report.clean.push(canonical);
                    }
                }
                Some(ReaperOutcome::WaitFailed(e)) => {
                    report.failed.push((
                        canonical,
                        ShutdownFailure::WaitFailed {
                            kind: e.kind(),
                            message: e.to_string(),
                        },
                    ));
                }
                Some(ReaperOutcome::ReaperPanicked) | None => {
                    report
                        .failed
                        .push((canonical, ShutdownFailure::ReaperPanicked));
                }
            }
        }

        report
    }
}

/// Append a single trace line `"<event> <canonical>\n"` to the file
/// named by `RFL_SPAWN_TRACE_LOG` if set. Caller-side emission only
/// (pi-5 B-5 exclusive): `run_chat`'s eager spawn sites emit
/// `eager_spawn`, [`PluginSupervisor::ensure_spawned`]'s lazy arm
/// emits `spawn_on_demand`, and the inner `spawn` method writes no
/// trace itself.
pub fn record_spawn_event(canonical: &CanonicalId, event: &str) {
    use std::io::Write;
    if let Ok(path) = std::env::var("RFL_SPAWN_TRACE_LOG") {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| writeln!(f, "{event} {canonical}"));
    }
}

impl Drop for PluginSupervisor {
    fn drop(&mut self) {
        let managed = std::mem::take(self.managed.get_mut());
        for (_canonical, mut spawn) in managed {
            if let Some(pid) = spawn.observation.cached_pid {
                match nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                ) {
                    Ok(()) | Err(nix::errno::Errno::ESRCH) => {}
                    Err(_) => {}
                }
            }
            if let Some(j) = spawn.serve_join.take() {
                j.abort();
            }
            drop(spawn.registered.take());
            drop(spawn.proxy.take());
        }
    }
}

/// RAII reservation for the supervisor `in_flight` set.
///
/// Acquired at SP4 step 1a; on every Phase A/B failure the
/// guard's `Drop` removes the canonical from `in_flight`,
/// allowing retry. On full spawn success the guard is dropped
/// after broker registration becomes the source of truth.
struct InFlightGuard {
    set: Arc<Mutex<HashSet<CanonicalId>>>,
    canonical: CanonicalId,
}

impl InFlightGuard {
    fn acquire(
        set: Arc<Mutex<HashSet<CanonicalId>>>,
        canonical: &CanonicalId,
    ) -> Result<Self, SpawnError> {
        let inserted = set.lock().insert(canonical.clone());
        if !inserted {
            return Err(SpawnError::AlreadyRegistered(canonical.clone()));
        }
        Ok(Self {
            set,
            canonical: canonical.clone(),
        })
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.set.lock().remove(&self.canonical);
    }
}

fn is_reserved_env(name: &str) -> bool {
    RESERVED_ENV_VARS.contains(&name)
}

fn validate_path(path: &Path, kind: PathKind, canonical: &CanonicalId) -> Result<(), SpawnError> {
    if !path.is_absolute() {
        return Err(SpawnError::InvalidPlan {
            canonical: canonical.clone(),
            reason: InvalidPlanReason::NonAbsolutePath {
                kind,
                path: path.to_path_buf(),
            },
        });
    }
    if path
        .as_os_str()
        .as_bytes()
        .iter()
        .any(|b| *b < 0x20 || *b == 0x7f)
    {
        return Err(SpawnError::InvalidPlan {
            canonical: canonical.clone(),
            reason: InvalidPlanReason::ControlCharsInPath {
                kind,
                path: path.to_path_buf(),
            },
        });
    }
    Ok(())
}

/// Per-connection `bus.publish` notification handler (scope §SP4
/// step 16, pi-1 c19). Real `fittings_core::service::Service` impl
/// — `bus.publish` notifications dispatch to the broker; every
/// other inbound method (and any `bus.publish` *request*) returns
/// `MethodNotFound`.
struct BusPublishService {
    broker: Broker,
    canonical: CanonicalId,
}

#[async_trait]
impl Service for BusPublishService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "bus.publish" && req.id.is_none() {
            // Notification: result is swallowed by the server; any
            // BrokerError is already surfaced via the broker's
            // lifecycle event emission (§B9). Provider-bound peers
            // route to `handle_provider_publish`; plugin peers route
            // to `handle_plugin_publish` (scope §M2.3).
            if self.broker.contains_provider(&self.canonical) {
                let _ = self
                    .broker
                    .handle_provider_publish(&self.canonical, &req.params);
            } else {
                let _ = self
                    .broker
                    .handle_plugin_publish(&self.canonical, &req.params);
            }
            return Ok(Response {
                id: JsonRpcId::Null,
                result: Value::Null,
                metadata: Default::default(),
            });
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

/// Routing facade for the per-connection service (scope §SP4 step
/// 16). In production the bus handler is the only branch; in test
/// mode the supervisor's `with_extra_service` factory contributes
/// additional services that handle every method other than
/// `bus.publish`.
struct SupervisorConnectionService {
    bus: BusPublishService,
    core: Option<CorePluginService>,
    #[cfg(any(test, feature = "test-fixture"))]
    extra: Option<Box<dyn Service + Send + Sync>>,
}

#[async_trait]
impl Service for SupervisorConnectionService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "bus.publish" {
            return self.bus.call(req, ctx).await;
        }
        if req.method == "core.tools_list" {
            if let Some(core) = &self.core {
                return core.call(req, ctx).await;
            }
            return Err(FittingsError::method_not_found(req.method));
        }
        #[cfg(any(test, feature = "test-fixture"))]
        if let Some(extra) = &self.extra {
            return extra.call(req, ctx).await;
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker_acl::BrokerAcl;
    use fittings_core::context::{DroppedNotifications, PeerHandle};
    use tokio::sync::watch;
    use tokio_util::sync::CancellationToken;

    fn synth_supervisor() -> PluginSupervisor {
        let broker = Broker::new(BrokerAcl::default()).expect("Broker::new");
        PluginSupervisor::new(
            broker,
            SupervisorConfig::default(),
            ToolSchemaCatalog::empty_for_tests(),
        )
    }

    fn synth_canonical(s: &str) -> CanonicalId {
        CanonicalId::parse(s).expect("canonical parses")
    }

    fn synth_managed(canonical: CanonicalId) -> ManagedSpawn {
        let (notify_tx, _notify_rx) = tokio::sync::mpsc::channel(1);
        let dropped = DroppedNotifications::new();
        let token = CancellationToken::new();
        let peer = PeerHandle::new(notify_tx, dropped, token);
        let (_w_tx, w_rx) = watch::channel::<Option<Arc<ReaperOutcome>>>(None);
        let observation = Arc::new(SpawnObservation {
            canonical: canonical.clone(),
            topic_id: "test-topic".to_string(),
            cached_pid: None,
            peer,
            outcome: w_rx,
        });
        ManagedSpawn {
            observation,
            registered: None,
            proxy: None,
            serve_join: None,
            watcher_join: None,
        }
    }

    #[tokio::test]
    async fn ensure_spawned_returns_ok_false_for_managed_canonical() {
        let sup = synth_supervisor();
        let canonical = synth_canonical("local/test:already@0.1.0");
        sup.managed
            .lock()
            .insert(canonical.clone(), synth_managed(canonical.clone()));
        let before = sup.managed.lock().len();

        let out = sup.ensure_spawned(&canonical).await.expect("ok");
        assert!(!out, "managed canonical → Ok(false)");
        assert_eq!(sup.managed.lock().len(), before);
    }

    #[tokio::test]
    async fn ensure_spawned_returns_ok_false_when_no_candidate() {
        let sup = synth_supervisor();
        let canonical = synth_canonical("local/test:eager@0.1.0");
        let out = sup.ensure_spawned(&canonical).await.expect("ok");
        assert!(!out, "no candidate → Ok(false) (pi-5 B-3 eager-tool no-op)");
        assert!(sup.managed.lock().is_empty());
        assert!(sup.lazy_candidates.lock().is_empty());
    }

    #[tokio::test]
    async fn ensure_spawned_dispatches_lazy_candidate_then_idempotent() {
        // The lazy-spawn happy-path (`Ok(true)` + `managed` populated)
        // requires a real lockin sandbox + child process, which lives
        // in the m2 integration-test harness. Here we cover the
        // adjacent invariants the prompt names:
        //   * registration installs the candidate + tool→canonical
        //     entry,
        //   * a spawn that fails restores the candidate to
        //     `lazy_candidates` (so a retry can still find it — pi-5
        //     B-4 idempotent recovery),
        //   * once `managed` is populated, the second call returns
        //     `Ok(false)` and leaves `managed` unchanged (pi-5 B-4
        //     idempotent re-dispatch).
        let sup = synth_supervisor();
        let canonical = synth_canonical("local/test:lazy@0.1.0");
        let plan = synth_plan(&canonical);
        let paths = SpawnPaths {
            project_root: PathBuf::from("/tmp"),
            private_state_dir: PathBuf::from("/tmp/rfl-test-state"),
        };
        sup.register_lazy(canonical.clone(), plan, paths, vec!["t1".to_string()]);
        assert!(sup.lazy_candidates.lock().contains_key(&canonical));
        assert_eq!(
            sup.tool_to_canonical.lock().get("t1"),
            Some(&canonical),
            "trigger registered in tool→canonical map",
        );

        // First call: candidate exists but broker has no ACL for this
        // canonical, so `spawn` returns `NotInAcl`; the candidate must
        // be restored.
        let err = sup
            .ensure_spawned(&canonical)
            .await
            .expect_err("spawn fails — broker has empty ACL");
        assert!(matches!(err, SpawnError::NotInAcl(_)));
        assert!(
            sup.lazy_candidates.lock().contains_key(&canonical),
            "candidate restored after spawn Err (pi-5 B-4 retry path)",
        );

        // Simulate a successful spawn by inserting into `managed`
        // directly, then assert the next call short-circuits with
        // `Ok(false)` and `managed` is unchanged.
        sup.lazy_candidates.lock().remove(&canonical);
        sup.managed
            .lock()
            .insert(canonical.clone(), synth_managed(canonical.clone()));
        let again = sup.ensure_spawned(&canonical).await.expect("ok");
        assert!(!again, "second call → Ok(false) (pi-5 B-4 idempotent)");
        assert_eq!(sup.managed.lock().len(), 1, "managed unchanged");
    }

    fn synth_plan(canonical: &CanonicalId) -> CompiledPlugin {
        use crate::compile::{CompiledFlags, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan};
        CompiledPlugin {
            canonical: canonical.clone(),
            topic_id: "test-topic".to_string(),
            entry_absolute: PathBuf::from("/bin/true"),
            filesystem: FilesystemPlan::default(),
            network: NetworkPlan::Deny,
            env: EnvPlan::default(),
            limits: LimitsPlan::default(),
            subscribe_patterns: Vec::new(),
            publish_topics: Vec::new(),
            auto_subscribes: Vec::new(),
            tool_meta: std::collections::BTreeMap::new(),
            provider_id: None,
            load: crate::lock::LoadPolicy::Lazy {
                event: Vec::new(),
                command: vec!["t1".to_string()],
                kind: Vec::new(),
            },
            flags: CompiledFlags::default(),
        }
    }
}
