//! c07 §H6.3 — arm the post-register fault hook (c06 §H6.2 third
//! inject point), call `PluginSupervisor::spawn`, and verify the
//! cross-platform unwind contract: spawn returns
//! `SpawnError::SandboxBuild`, the broker registration was rolled
//! back (canonical stays in the ACL but is reservable again), the
//! `in_flight` reservation drained, and no proxy was started.

#![cfg(feature = "test-fixture")]

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};
use rafaello_core::bus::Broker;
use rafaello_core::compile::NetworkPlan;
use rafaello_core::error::SpawnError;
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnPaths, SupervisorConfig, ToolSchemaCatalog,
};

#[tokio::test]
async fn spawn_unwinds_after_post_register_fault() {
    let mut built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("fixture-post-register", "ready_only"))
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
    hooks
        .inject_post_register_fault
        .store(true, Ordering::SeqCst);

    let paths = SpawnPaths {
        project_root: project_root.clone(),
        private_state_dir: project_root
            .join(".rafaello-plugin-data")
            .join(&plan.topic_id),
    };

    let canonical = plan.canonical.clone();
    let result = sup.spawn(&plan, &paths).await;
    match result {
        Err(SpawnError::SandboxBuild {
            canonical: c,
            source,
        }) => {
            assert_eq!(c, canonical);
            let msg = source.to_string();
            assert!(
                msg.contains("test-injected post-register fault"),
                "unexpected source: {msg}"
            );
        }
        Err(other) => panic!("expected SpawnError::SandboxBuild, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    assert!(hooks.post_register_fault_consumed());
    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.last_proxy_port.load(Ordering::SeqCst), 0);
    assert!(
        broker.contains_plugin(&canonical),
        "canonical must remain in the ACL after unwind"
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if broker.try_reserve_registration(&canonical).is_ok() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("RegisteredPlugin guard must drop and re-open the slot");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        !sup.is_in_flight(&canonical),
        "InFlightGuard must drop on post-register unwind"
    );
}
