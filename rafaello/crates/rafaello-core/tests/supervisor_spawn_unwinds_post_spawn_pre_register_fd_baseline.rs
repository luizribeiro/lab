//! c07 §H6.3 — Linux-only fd-baseline complement to
//! `supervisor_spawn_unwinds_post_spawn_pre_register`: arms the
//! post-spawn-pre-register fault and asserts that
//! `/proc/self/fd` returns to its pre-spawn baseline once the
//! failing call has unwound (covers the socketpair + child stdio
//! cleanup).

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::sync::atomic::Ordering;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};
use rafaello_core::bus::Broker;
use rafaello_core::compile::NetworkPlan;
use rafaello_core::error::SpawnError;
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};

fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .expect("read /proc/self/fd")
        .count()
}

#[tokio::test]
async fn post_spawn_pre_register_fault_returns_fd_count_to_baseline() {
    let mut built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("fixture-mid-fd", "ready_only"))
        .build();
    let project_root = built.layout.project.path().to_path_buf();
    let mut plan = built.plans.remove(0);
    plan.network = NetworkPlan::Deny;

    let broker = Broker::new(built.broker_acl).expect("Broker::new");
    let sup = PluginSupervisor::new(broker, SupervisorConfig::default());
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
    let baseline = open_fd_count();

    match sup.spawn(&plan, &paths).await {
        Err(SpawnError::SandboxBuild { canonical: c, .. }) => assert_eq!(c, canonical),
        Err(other) => panic!("expected SpawnError::SandboxBuild, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    assert!(hooks.post_spawn_pre_register_fault_consumed());

    let after = open_fd_count();
    assert_eq!(
        after, baseline,
        "open fd count should return to baseline after unwind \
         (baseline={baseline}, after={after})"
    );
}
