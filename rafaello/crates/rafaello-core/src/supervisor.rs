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

use parking_lot::Mutex;
use tokio::sync::watch;

use crate::bus::{Broker, PeerHandle, RegisteredPlugin};
use crate::compile::{CompiledPlugin, NetworkPlan};
use crate::error::{
    BrokerError, InvalidPlanReason, PathKind, ReaperOutcome, ShutdownFailure, SpawnError,
};
use crate::lock::CanonicalId;

use outpost_proxy::ProxyHandle;

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
/// supervisor owns the child via [`ManagedSpawn`] and kills
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
        self.0.cached_pid
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
    registered: Option<RegisteredPlugin>,
    proxy: Option<ProxyHandle>,
    serve_join: Option<tokio::task::JoinHandle<()>>,
    watcher_join: Option<tokio::task::JoinHandle<()>>,
}

/// Test-only counters (scope §SP2).
///
/// All wired so callers can read real values; later commits
/// increment them as the spawn pipeline lights up.
#[cfg(any(test, feature = "test-fixture"))]
#[derive(Debug, Default)]
pub struct TestHooks {
    pub outpost_starts: std::sync::atomic::AtomicUsize,
    pub socketpair_creates: std::sync::atomic::AtomicUsize,
    pub child_spawns: std::sync::atomic::AtomicUsize,
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
    #[cfg(any(test, feature = "test-fixture"))]
    test_hooks: Arc<TestHooks>,
    #[cfg(any(test, feature = "test-fixture"))]
    #[allow(dead_code)]
    extra_service_factory: Option<ExtraServiceFactory>,
}

impl PluginSupervisor {
    pub fn new(broker: Broker, config: SupervisorConfig) -> Self {
        Self {
            broker,
            config,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            managed: Mutex::new(BTreeMap::new()),
            #[cfg(any(test, feature = "test-fixture"))]
            test_hooks: Arc::new(TestHooks::default()),
            #[cfg(any(test, feature = "test-fixture"))]
            extra_service_factory: None,
        }
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn with_extra_service(
        broker: Broker,
        config: SupervisorConfig,
        factory: ExtraServiceFactory,
    ) -> Self {
        let mut s = Self::new(broker, config);
        s.extra_service_factory = Some(factory);
        s
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn test_hooks(&self) -> Arc<TestHooks> {
        Arc::clone(&self.test_hooks)
    }

    pub async fn spawn(
        &self,
        plan: &CompiledPlugin,
        paths: &SpawnPaths,
    ) -> Result<SpawnHandle, SpawnError> {
        let _in_flight = InFlightGuard::acquire(Arc::clone(&self.in_flight), &plan.canonical)?;

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

        if let Some(provider_id) = acl_provider_id {
            return Err(SpawnError::InvalidPlan {
                canonical: plan.canonical.clone(),
                reason: InvalidPlanReason::ProviderNotInM2 { provider_id },
            });
        }

        Err(SpawnError::SandboxBuild {
            canonical: plan.canonical.clone(),
            source: anyhow::anyhow!("Phase A step 8 not yet implemented"),
        })
    }

    pub async fn shutdown(self) -> ShutdownReport {
        ShutdownReport::default()
    }
}

impl Drop for PluginSupervisor {
    fn drop(&mut self) {
        // Real best-effort SIGKILL teardown lands in c26.
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

// Type plumbing: a watch channel initialised to `None` so
// `SpawnHandle::wait` is a usable type before any reaper
// task exists. c14's `spawn` never returns `Ok`, so no
// `SpawnHandle` is ever produced — this exercises the type
// contract only.
#[allow(dead_code)]
fn _observation_plumbing_compiles(
    canonical: CanonicalId,
    topic_id: String,
    peer: PeerHandle,
) -> Arc<SpawnObservation> {
    let (_tx, rx) = watch::channel(None);
    Arc::new(SpawnObservation {
        canonical,
        topic_id,
        cached_pid: None,
        peer,
        outcome: rx,
    })
}
