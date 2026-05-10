//! c25 SP5 cooperative shutdown — forced path. The fixture
//! installs a real SIGTERM handler (drains and ignores) when
//! `RFL_FIXTURE_TRAP_SIGTERM=1`. With a 50 ms grace, the
//! supervisor must escalate to SIGKILL and continue waiting on
//! the watch until the reaper observes the exit (pi-1 B7) so
//! `report.forced` reflects actual state and there is no zombie.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use fittings_core::{
    context::ServiceContext,
    error::FittingsError,
    message::{JsonRpcId, Request, Response},
    service::Service,
};
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::error::ReaperOutcome;
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};
use serde_json::Value;
use tokio::sync::mpsc;

fn runtime_filesystem(binary: &Path) -> FilesystemPlan {
    let mut exec_dirs: Vec<PathBuf> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if !d.as_os_str().is_empty() && d.is_absolute() {
                exec_dirs.push(d);
            }
        }
    }
    if exec_dirs.is_empty() {
        exec_dirs.push(PathBuf::from("/nix/store"));
    }
    if let Some(parent) = binary.parent() {
        exec_dirs.push(parent.to_path_buf());
    }
    FilesystemPlan {
        exec_dirs,
        ..FilesystemPlan::default()
    }
}

struct ReadyCaptureService {
    tx: mpsc::UnboundedSender<Value>,
}

#[async_trait]
impl Service for ReadyCaptureService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        if req.method == "core.fixture.ready" {
            let _ = self.tx.send(req.params);
            return Ok(Response {
                id,
                result: Value::Null,
                metadata: Default::default(),
            });
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_sigkill_after_grace_reaps_trapping_child() {
    let (ready_tx, mut ready_rx) = mpsc::unbounded_channel::<Value>();
    let factory = {
        let ready_tx = ready_tx.clone();
        Arc::new(move |_canonical: CanonicalId| {
            let svc: Box<dyn Service + Send + Sync> = Box::new(ReadyCaptureService {
                tx: ready_tx.clone(),
            });
            svc
        })
    };

    let canonical = CanonicalId::parse("local/test:plugin@0.1.0").unwrap();
    let real_topic = rafaello_core::topic_id::derive(&canonical.to_string());

    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: real_topic.clone(),
            publish_topics: Vec::new(),
            subscribe_patterns: Vec::new(),
            auto_subscribes: vec![format!("plugin.{real_topic}.tool_request")],
            provider_id: None,
        },
    );
    let broker = Broker::new(BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    })
    .unwrap();

    let grace = Duration::from_millis(50);
    let config = SupervisorConfig {
        shutdown_grace: grace,
        ..SupervisorConfig::default()
    };
    let sup = PluginSupervisor::with_extra_service(broker, config, factory);

    let proj = tempfile::tempdir().unwrap();
    let entry = PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"));
    let plan = CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: real_topic.clone(),
        entry_absolute: entry.clone(),
        filesystem: runtime_filesystem(&entry),
        network: NetworkPlan::AllowAll,
        env: EnvPlan {
            set: {
                let mut s = BTreeMap::new();
                s.insert("RFL_FIXTURE_MODE".into(), "respond_peer_call".into());
                s.insert("RFL_FIXTURE_TRAP_SIGTERM".into(), "1".into());
                s
            },
            pass: Vec::new(),
        },
        limits: LimitsPlan {
            max_cpu_time: 5,
            max_open_files: 64,
            max_address_space: None,
            max_processes: None,
        },
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta: BTreeMap::new(),
        provider_id: None,
        load: LoadPolicy::default(),
        flags: CompiledFlags::default(),
    };
    let paths = SpawnPaths {
        project_root: proj.path().to_path_buf(),
        private_state_dir: proj.path().join(".rafaello-plugin-data").join(&real_topic),
    };

    let handle = sup.spawn(&plan, &paths).await.expect("spawn ok");
    tokio::time::timeout(Duration::from_secs(5), ready_rx.recv())
        .await
        .expect("ready timed out")
        .expect("ready channel closed");

    let started = Instant::now();
    // Bounded outer wait — anything beyond a few seconds is a hang
    // (driver-notes c22 lesson). The forced path itself should
    // resolve well under `grace * 4`; we assert that lower bound
    // separately below.
    let report = tokio::time::timeout(Duration::from_secs(5), sup.shutdown())
        .await
        .expect("shutdown timed out");
    let elapsed = started.elapsed();

    assert!(
        report.forced.contains(&canonical),
        "expected canonical in forced, got report = {report:?}"
    );
    assert!(report.clean.is_empty(), "report.clean = {:?}", report.clean);
    assert!(
        report.failed.is_empty(),
        "report.failed = {:?}",
        report.failed
    );
    assert!(
        elapsed < grace * 4,
        "forced shutdown took {elapsed:?}, expected < {:?}",
        grace * 4
    );

    // pi-1 B7: SIGKILL'd child must be reaped before shutdown
    // returns. Verify the cached terminal outcome is present and
    // is `Exited` (the kernel surfaces SIGKILL via ExitStatus,
    // which still resolves as `Exited(_)` in our taxonomy).
    match handle.try_wait().as_deref() {
        Some(ReaperOutcome::Exited(_)) => {}
        other => panic!("expected Exited after forced shutdown, got {other:?}"),
    }
}
