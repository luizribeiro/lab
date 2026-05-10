//! c07 §H6.2 / §H6.3 — arms the post-spawn-pre-register fault
//! (between `tokio_command.spawn()` and `broker.register_plugin`).
//! Cross-platform: hook consumed, `SpawnError::SandboxBuild`
//! returned, broker remains reservable (no registration was
//! acquired), and `in_flight` drained.

#![cfg(feature = "test-fixture")]

mod common;

use std::sync::atomic::Ordering;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};
use rafaello_core::bus::Broker;
use rafaello_core::compile::NetworkPlan;
use rafaello_core::error::SpawnError;
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};

#[tokio::test]
async fn spawn_unwinds_post_spawn_pre_register_fault() {
    let mut built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("fixture-mid", "ready_only"))
        .build();
    let project_root = built.layout.project.path().to_path_buf();
    let mut plan = built.plans.remove(0);
    plan.network = NetworkPlan::Deny;

    let broker = Broker::new(built.broker_acl).expect("Broker::new");
    let sup = PluginSupervisor::new(broker.clone(), SupervisorConfig::default());
    let hooks = sup.test_hooks();
    hooks
        .inject_post_spawn_pre_register_fault
        .store(true, Ordering::SeqCst);

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
                msg.contains("test-injected post-spawn-pre-register fault"),
                "unexpected source: {msg}"
            );
        }
        Err(other) => panic!("expected SpawnError::SandboxBuild, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    assert!(hooks.post_spawn_pre_register_fault_consumed());
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.last_proxy_port.load(Ordering::SeqCst), 0);
    assert!(
        broker.try_reserve_registration(&canonical).is_ok(),
        "no registration should have been acquired during the failing spawn"
    );
    assert!(
        !sup.is_in_flight(&canonical),
        "InFlightGuard must drop on post-spawn-pre-register unwind"
    );
}
