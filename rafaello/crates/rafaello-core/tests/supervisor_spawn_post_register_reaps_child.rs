//! c07 §H6.3 — Linux-only complement to
//! `supervisor_spawn_unwinds_after_register`: arms the post-
//! register fault and asserts that the spawned child was reaped
//! via the supervisor's reaper task.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};
use rafaello_core::bus::Broker;
use rafaello_core::compile::NetworkPlan;
use rafaello_core::error::SpawnError;
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};

#[tokio::test]
async fn post_register_unwind_reaps_child_via_reaper() {
    let mut built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("fixture-reaps", "ready_only"))
        .build();
    let project_root = built.layout.project.path().to_path_buf();
    let mut plan = built.plans.remove(0);
    plan.network = NetworkPlan::Deny;

    let broker = Broker::new(built.broker_acl).expect("Broker::new");
    let sup = PluginSupervisor::new(broker, SupervisorConfig::default());
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
    match sup.spawn(&plan, &paths).await {
        Err(SpawnError::SandboxBuild { canonical: c, .. }) => assert_eq!(c, canonical),
        Err(other) => panic!("expected SpawnError::SandboxBuild, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    assert!(hooks.post_register_fault_consumed());
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 1);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let pid = loop {
        if let Some(p) = hooks.last_reaped_pid() {
            break p;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("supervisor must record the reaped pid in TestHooks");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    let status_path = format!("/proc/{pid}/status");
    match std::fs::metadata(&status_path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => panic!("pid {pid} is still alive after reap (/proc/{pid}/status exists)"),
        Err(e) => panic!("unexpected error stat-ing /proc/{pid}/status: {e}"),
    }
}
