//! c15 SP4 Phase A step 3a — a relative `entry_absolute`
//! surfaces as `InvalidPlan { reason: NonAbsolutePath { kind:
//! EntryAbsolute, .. } }`.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::error::{InvalidPlanReason, PathKind};
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnPaths, SupervisorConfig, ToolSchemaCatalog,
};
use rafaello_core::{topic_id, SpawnError};

#[tokio::test]
async fn spawn_with_relative_entry_absolute_returns_non_absolute_path() {
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
        frontends: BTreeMap::new(),
    })
    .unwrap();
    let sup = PluginSupervisor::new(
        broker,
        SupervisorConfig::default(),
        ToolSchemaCatalog::empty_for_tests(),
    );
    let hooks = sup.test_hooks();

    let bad_entry = PathBuf::from("relative/bin/fixture");
    let plan = CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: real_topic,
        entry_absolute: bad_entry.clone(),
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::default(),
        env: EnvPlan::default(),
        limits: LimitsPlan::default(),
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta: BTreeMap::new(),
        provider_id: None,
        load: LoadPolicy::default(),
        flags: CompiledFlags::default(),
    };
    let paths = SpawnPaths {
        project_root: PathBuf::from("/tmp/proj"),
        private_state_dir: PathBuf::from("/tmp/proj/.priv"),
    };

    let err = match sup.spawn(&plan, &paths).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        SpawnError::InvalidPlan {
            canonical: c,
            reason:
                InvalidPlanReason::NonAbsolutePath {
                    kind: PathKind::EntryAbsolute,
                    path,
                },
        } => {
            assert_eq!(c, canonical);
            assert_eq!(path, bad_entry);
        }
        other => panic!("expected NonAbsolutePath{{EntryAbsolute}}, got {other:?}"),
    }

    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 0);
}
