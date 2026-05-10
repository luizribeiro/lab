//! m2 integration-test harness per scope §H1–§H5.
//!
//! Builds a tempdir-backed project with one or more fixture-binary
//! plugin packages, materialises a real `Lock` value through the m1
//! public API (digest::content_digest + digest::manifest_digest +
//! validate::lock + compile_plugin + broker_acl::compile), then
//! launches a `PluginSupervisor::with_extra_service` whose
//! `core.fixture.*` extra services close the readiness, observer,
//! and ping-from-plugin loops the fixture binary's modes need.
//!
//! Test-only; gated on the `test-fixture` feature so the
//! production `rafaello-core` API surface is unaffected.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use fittings_core::context::ServiceContext;
use fittings_core::error::FittingsError;
use fittings_core::message::{JsonRpcId, Request, Response};
use fittings_core::service::Service;
use parking_lot::Mutex as PlMutex;
use serde_json::Value;
use tempfile::TempDir;
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use rafaello_core::broker_acl::{self, BrokerAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{compile_plugin, CompiledPlugin, NetworkPlan};
use rafaello_core::digest::{content_digest, manifest_digest, RecomputedDigests};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantFilesystem, GrantNetwork, Lock, LockFlags,
    PluginEntry, SessionTable,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::{Manifest, SafePath};
use rafaello_core::paths::PathContext;
use rafaello_core::supervisor::{
    ExtraServiceFactory, PluginSupervisor, SpawnHandle, SpawnPaths, SupervisorConfig,
};
use rafaello_core::topic_id;
use rafaello_core::validate::{self, LockValidationContext};

const EMPTY_OPENRPC: &str = include_str!("empty_openrpc.json");

/// Multi-namespace observer grant per scope §H4 (NEVER bare `**`).
pub fn observer_subscribe_patterns() -> Vec<String> {
    vec![
        "core.**".to_string(),
        "plugin.**".to_string(),
        "provider.**".to_string(),
        "frontend.**".to_string(),
    ]
}

/// One fixture's input to the harness builder.
#[derive(Debug, Clone)]
pub struct FixtureSpec {
    pub canonical: CanonicalId,
    pub mode: String,
    pub publishes: Vec<String>,
    pub subscribes: Vec<String>,
    pub env_set: BTreeMap<String, String>,
    pub network_plan: Option<NetworkPlan>,
}

impl FixtureSpec {
    pub fn new(name: &str, mode: &str) -> Self {
        let canonical =
            CanonicalId::parse(&format!("local/test:{name}@0.1.0")).expect("canonical id parses");
        let mut env_set = BTreeMap::new();
        env_set.insert("RFL_FIXTURE_MODE".to_string(), mode.to_string());
        Self {
            canonical,
            mode: mode.to_string(),
            publishes: Vec::new(),
            subscribes: Vec::new(),
            env_set,
            network_plan: None,
        }
    }

    pub fn with_network_plan(mut self, plan: NetworkPlan) -> Self {
        self.network_plan = Some(plan);
        self
    }

    pub fn topic_id(&self) -> String {
        topic_id::derive(&self.canonical.to_string())
    }

    pub fn publishes(mut self, topics: Vec<String>) -> Self {
        self.publishes = topics;
        self
    }

    pub fn subscribes(mut self, patterns: Vec<String>) -> Self {
        self.subscribes = patterns;
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env_set.insert(key.to_string(), value.to_string());
        self
    }
}

/// Materialises plugin packages and constructs the m1 Lock + plans
/// (scope §H2). Caller submits one [`FixtureSpec`] per fixture
/// instance.
pub struct FixtureLockBuilder {
    specs: Vec<FixtureSpec>,
}

impl Default for FixtureLockBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FixtureLockBuilder {
    pub fn new() -> Self {
        Self { specs: Vec::new() }
    }

    pub fn add(mut self, spec: FixtureSpec) -> Self {
        self.specs.push(spec);
        self
    }

    pub fn build(self) -> BuiltFixtures {
        let project = tempfile::tempdir().expect("project tempdir");
        let project_root = project.path().to_path_buf();
        let plugins_root = project_root.join("plugins");
        std::fs::create_dir_all(&plugins_root).expect("create plugins/");

        let bin_path = PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"));
        let bin_bytes = std::fs::read(&bin_path).expect("read fixture binary");

        let mut plugin_dirs: BTreeMap<CanonicalId, PathBuf> = BTreeMap::new();
        let mut manifests: BTreeMap<CanonicalId, String> = BTreeMap::new();

        for spec in &self.specs {
            let pdir = plugins_root.join(spec.canonical.name());
            let bin_dir = pdir.join("bin");
            std::fs::create_dir_all(&bin_dir).expect("create plugin bin/");
            let dst = bin_dir.join("fixture");
            std::fs::write(&dst, &bin_bytes).expect("copy fixture binary");
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod fixture binary");
            }
            std::fs::write(pdir.join("openrpc.json"), EMPTY_OPENRPC)
                .expect("write openrpc.json sibling");
            let manifest_text = format!(
                "schema = 1\n\
                 name = \"{name}\"\n\
                 version = \"0.1.0\"\n\
                 entry = \"bin/fixture\"\n\
                 rafaello = \">=0.0.0\"\n",
                name = spec.canonical.name(),
            );
            std::fs::write(pdir.join("rafaello.toml"), &manifest_text)
                .expect("write rafaello.toml");

            plugin_dirs.insert(spec.canonical.clone(), pdir);
            manifests.insert(spec.canonical.clone(), manifest_text);
        }

        let mut entries: BTreeMap<CanonicalId, PluginEntry> = BTreeMap::new();
        for spec in &self.specs {
            let pdir = plugin_dirs
                .get(&spec.canonical)
                .expect("plugin_dir present");
            let content = content_digest(pdir).expect("content_digest");
            let canonical_bytes = Manifest::parse(manifests.get(&spec.canonical).unwrap())
                .expect("manifest parses")
                .canonical_bytes();
            let m_digest = manifest_digest(&canonical_bytes);

            let mut bundles = BTreeMap::new();
            bundles.insert(
                "default".to_string(),
                GrantBundle {
                    filesystem: Some(GrantFilesystem {
                        exec_dirs: runtime_exec_dirs(),
                        ..GrantFilesystem::default()
                    }),
                    network: Some(GrantNetwork {
                        mode: NetworkMode::AllowAll,
                        allow_hosts: Vec::new(),
                    }),
                    ..GrantBundle::default()
                },
            );

            let entry = PluginEntry {
                entry: SafePath::parse("bin/fixture").expect("safepath"),
                digest: content,
                manifest_digest: m_digest,
                granted_at: Utc::now(),
                grant: Grant {
                    bundles,
                    publishes: spec.publishes.clone(),
                    subscribes: spec.subscribes.clone(),
                },
                bindings: Bindings::default(),
                flags: LockFlags::default(),
            };
            entries.insert(spec.canonical.clone(), entry);
        }

        let lock = Lock {
            plugins: entries,
            session: SessionTable::default(),
        };

        let lvc = LockValidationContext {
            project_root: project_root.clone(),
            home: project_root.clone(),
            plugin_dirs: plugin_dirs.clone(),
            cache_root: project_root.clone(),
            state_root: project_root.clone(),
        };
        validate::lock(&lock, &lvc).expect("validate::lock");

        let acl = broker_acl::compile(&lock).expect("broker_acl::compile");

        let mut plans: Vec<CompiledPlugin> = Vec::new();
        for canonical in lock.plugins.keys() {
            let pdir = plugin_dirs
                .get(canonical)
                .expect("plugin_dir present")
                .clone();
            let pctx = PathContext {
                project_root: project_root.clone(),
                home: project_root.clone(),
                plugin_dir: pdir.clone(),
                cache_dir: project_root.clone(),
                state_dir: project_root.clone(),
            };
            let recomputed = RecomputedDigests {
                content: content_digest(&pdir).expect("recompute content"),
                manifest: manifest_digest(
                    &Manifest::parse(manifests.get(canonical).unwrap())
                        .unwrap()
                        .canonical_bytes(),
                ),
            };
            let mut plan =
                compile_plugin(&lock, canonical, &pctx, &recomputed).expect("compile_plugin");
            let spec = self
                .specs
                .iter()
                .find(|s| &s.canonical == canonical)
                .expect("spec for canonical");
            for (k, v) in &spec.env_set {
                plan.env.set.insert(k.clone(), v.clone());
            }
            if let Some(np) = &spec.network_plan {
                plan.network = np.clone();
            }
            plans.push(plan);
        }

        BuiltFixtures {
            layout: ProjectLayout {
                project,
                plugin_dirs,
            },
            broker_acl: acl,
            plans,
        }
    }
}

fn runtime_exec_dirs() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if d.is_absolute() {
                out.push(d.to_string_lossy().into_owned());
            }
        }
    }
    if out.is_empty() {
        out.push("/nix/store".to_string());
    }
    out
}

pub struct ProjectLayout {
    pub project: TempDir,
    pub plugin_dirs: BTreeMap<CanonicalId, PathBuf>,
}

pub struct BuiltFixtures {
    pub layout: ProjectLayout,
    pub broker_acl: BrokerAcl,
    pub plans: Vec<CompiledPlugin>,
}

#[derive(Debug, Clone)]
pub struct ReadyMsg {
    pub canonical: CanonicalId,
    pub mode: String,
}

/// Captures `core.fixture.ready` calls from each fixture (scope §H5
/// fixture-ready half) and unblocks per-canonical waits.
pub struct ReadinessGate {
    rx: AsyncMutex<mpsc::UnboundedReceiver<ReadyMsg>>,
    seen: PlMutex<Vec<ReadyMsg>>,
}

impl ReadinessGate {
    pub async fn wait_for(&self, canonical: &CanonicalId, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let seen = self.seen.lock();
                if seen.iter().any(|r| &r.canonical == canonical) {
                    return;
                }
            }
            let mut guard = self.rx.lock().await;
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                panic!("ReadinessGate: timed out waiting for {canonical}");
            }
            match tokio::time::timeout(remaining, guard.recv()).await {
                Ok(Some(msg)) => {
                    let matches = &msg.canonical == canonical;
                    self.seen.lock().push(msg);
                    if matches {
                        return;
                    }
                }
                Ok(None) => panic!("ReadinessGate: channel closed"),
                Err(_) => panic!("ReadinessGate: timed out waiting for {canonical}"),
            }
        }
    }
}

/// Drains `core.fixture.observed` payloads forwarded by observer
/// fixtures (scope §H4).
pub struct Observer {
    rx: AsyncMutex<mpsc::UnboundedReceiver<Value>>,
}

impl Observer {
    /// The multi-namespace grant an observer plugin should be
    /// configured with. NEVER bare `**` per pi-1 §33, §463.
    pub fn watch_all() -> Vec<String> {
        observer_subscribe_patterns()
    }

    pub async fn next_event(&self, timeout: Duration) -> Value {
        let mut guard = self.rx.lock().await;
        tokio::time::timeout(timeout, guard.recv())
            .await
            .expect("Observer::next_event timed out")
            .expect("Observer channel closed")
    }
}

/// Synchronous handler the harness invokes when a plugin calls
/// `core.fixture.ping` (scope §C.4 supervisor_peer_call_plugin_to_core).
pub type PingHandler = Arc<dyn Fn(Value) -> Value + Send + Sync>;

struct HarnessExtraService {
    canonical: CanonicalId,
    ready_tx: mpsc::UnboundedSender<ReadyMsg>,
    observed_tx: mpsc::UnboundedSender<Value>,
    ping: Option<PingHandler>,
}

#[async_trait]
impl Service for HarnessExtraService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        let result = match req.method.as_str() {
            "core.fixture.ready" => {
                let mode = req
                    .params
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let _ = self.ready_tx.send(ReadyMsg {
                    canonical: self.canonical.clone(),
                    mode,
                });
                Value::Null
            }
            "core.fixture.observed" => {
                let _ = self.observed_tx.send(req.params);
                Value::Null
            }
            "core.fixture.after_publish" => Value::Null,
            "core.fixture.ping" => match &self.ping {
                Some(handler) => handler(req.params),
                None => return Err(FittingsError::method_not_found(req.method)),
            },
            other => return Err(FittingsError::method_not_found(other)),
        };
        Ok(Response {
            id,
            result,
            metadata: Default::default(),
        })
    }
}

/// Optional knobs for [`Spawn::launch`].
#[derive(Default, Clone)]
pub struct SpawnOptions {
    pub ping: Option<PingHandler>,
}

/// Result of [`Spawn::launch`] (scope §H3): broker, supervisor,
/// per-canonical spawn handles, plus the harness-side gate +
/// observer.
pub struct Harness {
    pub broker: Broker,
    pub supervisor: PluginSupervisor,
    pub handles: BTreeMap<CanonicalId, SpawnHandle>,
    pub readiness: Arc<ReadinessGate>,
    pub observer: Arc<Observer>,
    pub layout: ProjectLayout,
}

impl Harness {
    /// Best-effort SIGKILL every still-running fixture child. Tests
    /// call this in cleanup so worktree-removal can't strand
    /// fixture procs (driver-notes 2026-05-09).
    pub fn kill_all(&self) {
        for h in self.handles.values() {
            if let Some(pid) = h.child_pid() {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
        }
    }

    /// Await the reaper outcome on every handle (post-kill).
    pub async fn wait_all(&self) {
        for h in self.handles.values() {
            let _ = tokio::time::timeout(Duration::from_secs(5), h.wait()).await;
        }
    }
}

pub struct Spawn;

impl Spawn {
    pub async fn launch(built: BuiltFixtures, opts: SpawnOptions) -> Harness {
        let (ready_tx, ready_rx) = mpsc::unbounded_channel::<ReadyMsg>();
        let (observed_tx, observed_rx) = mpsc::unbounded_channel::<Value>();

        let readiness = Arc::new(ReadinessGate {
            rx: AsyncMutex::new(ready_rx),
            seen: PlMutex::new(Vec::new()),
        });
        let observer = Arc::new(Observer {
            rx: AsyncMutex::new(observed_rx),
        });

        let ping = opts.ping.clone();
        let factory: ExtraServiceFactory = Arc::new(move |canonical: CanonicalId| {
            let svc: Box<dyn Service + Send + Sync> = Box::new(HarnessExtraService {
                canonical,
                ready_tx: ready_tx.clone(),
                observed_tx: observed_tx.clone(),
                ping: ping.clone(),
            });
            svc
        });

        let broker = Broker::new(built.broker_acl).expect("Broker::new");
        let supervisor = PluginSupervisor::with_extra_service(
            broker.clone(),
            SupervisorConfig::default(),
            factory,
        );

        let project_root = built.layout.project.path().to_path_buf();
        let mut handles: BTreeMap<CanonicalId, SpawnHandle> = BTreeMap::new();
        for plan in built.plans {
            let paths = SpawnPaths {
                project_root: project_root.clone(),
                private_state_dir: project_root
                    .join(".rafaello-plugin-data")
                    .join(&plan.topic_id),
            };
            let canonical = plan.canonical.clone();
            let h = supervisor
                .spawn(&plan, &paths)
                .await
                .expect("supervisor.spawn");
            handles.insert(canonical, h);
        }

        Harness {
            broker,
            supervisor,
            handles,
            readiness,
            observer,
            layout: built.layout,
        }
    }
}
