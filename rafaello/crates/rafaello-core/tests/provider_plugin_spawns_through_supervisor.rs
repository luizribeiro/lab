//! c14 successor to the deleted m2 negative test
//! `supervisor_spawn_provider_lock_refused.rs`. Per the
//! synthetic-stub-successor pattern in `plans/README.md`: m2's
//! row-39 refusal is gone; this positive test exercises the
//! provider broker-registration branch (scope §M2.2) and the
//! `bus.publish` provider-dispatch path (scope §M2.3).

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::context::ServiceContext;
use fittings_core::error::FittingsError;
use fittings_core::message::{JsonRpcId, Request, Response};
use fittings_core::service::Service;
use serde_json::Value;
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::error::ReaperOutcome;
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{
    ExtraServiceFactory, PluginSupervisor, SpawnPaths, SupervisorConfig,
};
use rafaello_core::topic_id;

struct ReadyService {
    canonical: CanonicalId,
    ready_tx: mpsc::UnboundedSender<CanonicalId>,
}

#[async_trait]
impl Service for ReadyService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        match req.method.as_str() {
            "core.fixture.ready" => {
                let _ = self.ready_tx.send(self.canonical.clone());
                Ok(Response {
                    id,
                    result: Value::Null,
                    metadata: Default::default(),
                })
            }
            "core.fixture.after_publish" => Ok(Response {
                id,
                result: Value::Null,
                metadata: Default::default(),
            }),
            other => Err(FittingsError::method_not_found(other)),
        }
    }
}

fn runtime_exec_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if d.is_absolute() {
                out.push(d);
            }
        }
    }
    if out.is_empty() {
        out.push(PathBuf::from("/nix/store"));
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_plugin_spawns_through_supervisor() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let project = tempfile::tempdir().expect("project tempdir");
    let project_root = project.path().to_path_buf();
    let bin_src = PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"));
    let bin_dst = project_root.join("fixture");
    std::fs::copy(&bin_src, &bin_dst).expect("copy fixture binary");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_dst, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fixture binary");
    }

    let canonical = CanonicalId::parse("local/test:provider@0.1.0").unwrap();
    let real_topic = topic_id::derive(&canonical.to_string());
    let provider_id = "mock";
    let publish_topic = format!("provider.{provider_id}.tool_request");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: real_topic.clone(),
            publish_topics: vec![publish_topic.clone()],
            subscribe_patterns: Vec::new(),
            auto_subscribes: Vec::new(),
            provider_id: Some(provider_id.to_string()),
        },
    );
    let broker = Broker::new(BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    })
    .unwrap();

    let (mut event_rx, _internal_sub) = broker.subscribe_internal(vec![publish_topic.clone()], 16);

    let (ready_tx, mut ready_rx) = mpsc::unbounded_channel::<CanonicalId>();
    let factory: ExtraServiceFactory = {
        let ready_tx = ready_tx.clone();
        Arc::new(move |canonical: CanonicalId| {
            let svc: Box<dyn Service + Send + Sync> = Box::new(ReadyService {
                canonical,
                ready_tx: ready_tx.clone(),
            });
            svc
        })
    };
    let sup =
        PluginSupervisor::with_extra_service(broker.clone(), SupervisorConfig::default(), factory);

    let mut env_set = BTreeMap::new();
    env_set.insert(
        "RFL_FIXTURE_MODE".to_string(),
        "provider_bus_publish".to_string(),
    );
    env_set.insert("RFL_FIXTURE_MAX_LIFETIME".to_string(), "30".to_string());

    let plan = CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: real_topic.clone(),
        entry_absolute: bin_dst.clone(),
        filesystem: FilesystemPlan {
            exec_dirs: runtime_exec_dirs(),
            ..FilesystemPlan::default()
        },
        network: NetworkPlan::AllowAll,
        env: EnvPlan {
            pass: Vec::new(),
            set: env_set,
        },
        limits: LimitsPlan {
            max_cpu_time: 300,
            max_open_files: 1024,
            max_address_space: None,
            max_processes: None,
        },
        subscribe_patterns: Vec::new(),
        publish_topics: vec![publish_topic.clone()],
        auto_subscribes: Vec::new(),
        tool_meta: BTreeMap::new(),
        provider_id: Some(provider_id.to_string()),
        load: LoadPolicy::default(),
        flags: CompiledFlags::default(),
    };
    let paths = SpawnPaths {
        project_root: project_root.clone(),
        private_state_dir: project_root.join(".rafaello-plugin-data").join(&real_topic),
    };

    let handle = sup.spawn(&plan, &paths).await.expect("supervisor.spawn");

    assert!(
        broker.contains_provider(&canonical),
        "broker should report the provider as registered while the spawn is live"
    );
    assert!(
        handle.try_wait().is_none(),
        "freshly-spawned provider should not have a terminal outcome yet"
    );

    let cached_pid = handle.child_pid().expect("child_pid available");

    // Drain readiness.
    let ready = AsyncMutex::new(&mut ready_rx);
    let got = tokio::time::timeout(Duration::from_secs(15), async {
        ready.lock().await.recv().await
    })
    .await
    .expect("ready timeout")
    .expect("ready channel closed");
    assert_eq!(got, canonical);

    // Trigger the publish.
    handle
        .peer()
        .call("core.fixture.start", serde_json::json!({}))
        .await
        .expect("start ack");

    // Internal subscriber must receive the provider event via the
    // new BusPublishService dispatch path.
    let event = tokio::time::timeout(Duration::from_secs(15), event_rx.recv())
        .await
        .expect("event timeout")
        .expect("event channel closed");
    assert_eq!(event.topic, publish_topic);
    assert_eq!(
        event.payload,
        serde_json::json!({"tool": "noop", "args": {}})
    );

    // Tear the child down so SpawnHandle::wait resolves.
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(cached_pid as i32),
        nix::sys::signal::Signal::SIGKILL,
    );
    let outcome = tokio::time::timeout(Duration::from_secs(5), handle.wait())
        .await
        .expect("reaper timeout");
    assert!(
        matches!(*outcome, ReaperOutcome::Exited(_)),
        "expected Exited outcome, got {outcome:?}"
    );

    drop(sup);
}
