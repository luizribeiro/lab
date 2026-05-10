//! c25 SP5 cooperative shutdown — clean path. Spawns the
//! `respond_peer_call` fixture, waits for ready, then triggers
//! a clean child exit out-of-band before invoking
//! `PluginSupervisor::shutdown`. The reaper has already
//! published `Exited` to the watch, so shutdown's drain loop
//! observes the cached terminal outcome on first inspection,
//! skips signal delivery, and records the canonical in
//! `report.clean`. Exercises the "no SIGKILL escalation needed"
//! branch of the SP5 algorithm.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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
async fn shutdown_records_clean_when_child_already_exited() {
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
    let sup = PluginSupervisor::with_extra_service(broker, SupervisorConfig::default(), factory);

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

    // Drive the child to a clean exit out-of-band by SIGKILL'ing
    // the wrapper. The reaper task will publish `Exited(_)` to the
    // shared watch before we call shutdown, exercising the
    // already-terminated branch of the SP5 drain loop.
    let pid = handle.child_pid().expect("pid cached pre-exit");
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .expect("sigkill");
    let outcome = tokio::time::timeout(Duration::from_secs(5), handle.wait())
        .await
        .expect("wait timed out");
    assert!(matches!(&*outcome, ReaperOutcome::Exited(_)));

    let report = tokio::time::timeout(Duration::from_secs(5), sup.shutdown())
        .await
        .expect("shutdown timed out");

    assert!(
        report.clean.contains(&canonical),
        "expected canonical in clean, got report = {report:?}"
    );
    assert!(
        report.forced.is_empty(),
        "report.forced = {:?}",
        report.forced
    );
    assert!(
        report.failed.is_empty(),
        "report.failed = {:?}",
        report.failed
    );
}
