//! c16 SP4 Phase A step 4 — `env.pass` entries equal to one of
//! the six reserved RFL_* env names yield
//! `SpawnError::ReservedEnvInPlan { var }`. Counters stay at 0.

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
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnPaths, SupervisorConfig, ToolSchemaCatalog,
};
use rafaello_core::{topic_id, SpawnError};

const RESERVED: &[&str] = &[
    "RFL_BUS_FD",
    "RFL_PLUGIN",
    "RFL_HELPER_FD",
    "RFL_PROJECT_ROOT",
    "RFL_PRIVATE_STATE_DIR",
    "RFL_TOPIC_ID",
];

#[tokio::test]
async fn spawn_with_reserved_env_in_pass_returns_reserved_env_in_plan() {
    for &reserved in RESERVED {
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

        let plan = CompiledPlugin {
            canonical: canonical.clone(),
            topic_id: real_topic,
            entry_absolute: PathBuf::from("/usr/bin/true"),
            filesystem: FilesystemPlan::default(),
            network: NetworkPlan::default(),
            env: EnvPlan {
                pass: vec![reserved.to_string()],
                set: BTreeMap::new(),
            },
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
            Ok(_) => panic!("expected error for {reserved}"),
            Err(e) => e,
        };
        match err {
            SpawnError::ReservedEnvInPlan { canonical: c, var } => {
                assert_eq!(c, canonical);
                assert_eq!(var, reserved);
            }
            other => panic!("expected ReservedEnvInPlan for {reserved}, got {other:?}"),
        }

        assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
        assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 0);
        assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 0);
    }
}
