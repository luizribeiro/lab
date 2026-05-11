//! c07 §H6.3 — arm the pre-spawn fault hook (after socketpair /
//! proxy / sandbox-builder allocation, BEFORE `tokio_command.spawn()`)
//! and verify the cross-platform unwind contract: spawn returns
//! `SpawnError::SandboxBuild`, no child was created, no proxy was
//! started, the broker was never registered, and `in_flight` was
//! cleared.

#![cfg(feature = "test-fixture")]

mod common;

use std::sync::atomic::Ordering;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};
use rafaello_core::bus::Broker;
use rafaello_core::compile::NetworkPlan;
use rafaello_core::error::SpawnError;
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnPaths, SupervisorConfig, ToolSchemaCatalog,
};

#[tokio::test]
async fn spawn_unwinds_after_socketpair_pre_spawn_fault() {
    let mut built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("fixture-pre-spawn", "ready_only"))
        .build();
    let project_root = built.layout.project.path().to_path_buf();
    let mut plan = built.plans.remove(0);
    plan.network = NetworkPlan::Deny;

    let broker = Broker::new(built.broker_acl).expect("Broker::new");
    let sup = PluginSupervisor::new(
        broker.clone(),
        SupervisorConfig::default(),
        ToolSchemaCatalog::empty_for_tests(),
    );
    let hooks = sup.test_hooks();
    hooks.inject_pre_spawn_fault.store(true, Ordering::SeqCst);

    let paths = SpawnPaths {
        project_root: project_root.clone(),
        private_state_dir: project_root
            .join(".rafaello-plugin-data")
            .join(&plan.topic_id),
    };

    let canonical = plan.canonical.clone();
    match sup.spawn(&plan, &paths).await {
        Err(SpawnError::SandboxBuild {
            canonical: c,
            source,
        }) => {
            assert_eq!(c, canonical);
            let msg = source.to_string();
            assert!(
                msg.contains("test-injected pre-spawn fault"),
                "unexpected source: {msg}"
            );
        }
        Err(other) => panic!("expected SpawnError::SandboxBuild, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    assert!(hooks.pre_spawn_fault_consumed());
    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.last_proxy_port.load(Ordering::SeqCst), 0);
    assert!(
        broker.try_reserve_registration(&canonical).is_ok(),
        "no registration should have been acquired during the failing spawn"
    );
    assert!(
        !sup.is_in_flight(&canonical),
        "InFlightGuard must drop on pre-spawn unwind"
    );
}
