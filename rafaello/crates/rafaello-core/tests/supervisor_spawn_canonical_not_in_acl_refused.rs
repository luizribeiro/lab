//! c15 SP4 Phase A step 1b — `try_reserve_registration` with
//! a canonical missing from the broker ACL surfaces as
//! `SpawnError::NotInAcl`. No resource counters tick.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnPaths, SupervisorConfig, ToolSchemaCatalog,
};
use rafaello_core::{topic_id, SpawnError};

#[tokio::test]
async fn spawn_with_canonical_not_in_acl_returns_not_in_acl() {
    let canonical = CanonicalId::parse("local/test:plugin@0.1.0").unwrap();
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).unwrap();
    let sup = PluginSupervisor::new(
        broker,
        SupervisorConfig::default(),
        ToolSchemaCatalog::empty_for_tests(),
    );
    let hooks = sup.test_hooks();

    let plan = CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: topic_id::derive(&canonical.to_string()),
        entry_absolute: PathBuf::from("/usr/bin/true"),
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
    assert!(
        matches!(&err, SpawnError::NotInAcl(c) if c == &canonical),
        "expected NotInAcl, got {err:?}",
    );

    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 0);
}
