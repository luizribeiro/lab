//! c19 SP4 Phase B steps 13–17 — spawn proceeds through child
//! spawn, transport setup, and broker registration, then bails
//! at the step-18+ stub. This test asserts the full post-step-13
//! unwind: SIGKILL+reap of the child, the `RegisteredPlugin`
//! guard is dropped (broker registry returns to "reservable"),
//! the in-flight reservation drains, and counters increment
//! exactly once.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};
use rafaello_core::{topic_id, SpawnError};

#[tokio::test]
async fn spawn_unwinds_after_register_and_drops_in_flight() {
    let entry = PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"));
    let canonical = CanonicalId::parse("local/test:plugin@0.1.0").unwrap();
    let real_topic = topic_id::derive(&canonical.to_string());

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
    })
    .unwrap();
    let sup = PluginSupervisor::new(broker.clone(), SupervisorConfig::default());
    let hooks = sup.test_hooks();

    let proj = tempfile::tempdir().unwrap();
    let plan = CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: real_topic.clone(),
        entry_absolute: entry,
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::Deny,
        env: EnvPlan::default(),
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

    let err = match sup.spawn(&plan, &paths).await {
        Ok(_) => panic!("expected step-18+ stub error"),
        Err(e) => e,
    };
    match err {
        SpawnError::SandboxBuild {
            canonical: c,
            source,
        } => {
            assert_eq!(c, canonical);
            let msg = format!("{source}");
            assert!(
                msg.contains("Phase B step 18+"),
                "expected step-18+ stub message, got: {msg}"
            );
        }
        other => panic!("expected SandboxBuild step-18+ stub, got {other:?}"),
    }

    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 1);

    assert!(
        broker.contains_plugin(&canonical),
        "canonical must remain in ACL after unwind"
    );
    assert!(
        broker.try_reserve_registration(&canonical).is_ok(),
        "RegisteredPlugin drop must roll back the live registration"
    );
    assert!(
        !sup.is_in_flight(&canonical),
        "InFlightGuard drop must drain canonical from in_flight set"
    );
}
