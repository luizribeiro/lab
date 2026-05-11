//! c06 §H6.1 + §H6.2 — `TestHooks` exposes three one-shot fault
//! injection points (pre-spawn / post-spawn-pre-register /
//! post-register). Each, when armed, causes
//! `PluginSupervisor::spawn` to return `SpawnError::SandboxBuild`
//! and toggles its matching `*_consumed` accessor exactly once.

#![cfg(feature = "test-fixture")]

mod common;

use std::sync::atomic::Ordering;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};
use rafaello_core::bus::Broker;
use rafaello_core::error::SpawnError;
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnPaths, SupervisorConfig, TestHooks, ToolSchemaCatalog,
};

struct Case {
    label: &'static str,
    arm: fn(&TestHooks),
    consumed: fn(&TestHooks) -> bool,
    expected_fragment: &'static str,
}

const CASES: &[Case] = &[
    Case {
        label: "pre-spawn",
        arm: |h| h.inject_pre_spawn_fault.store(true, Ordering::SeqCst),
        consumed: |h| h.pre_spawn_fault_consumed(),
        expected_fragment: "test-injected pre-spawn fault",
    },
    Case {
        label: "post-spawn-pre-register",
        arm: |h| {
            h.inject_post_spawn_pre_register_fault
                .store(true, Ordering::SeqCst)
        },
        consumed: |h| h.post_spawn_pre_register_fault_consumed(),
        expected_fragment: "test-injected post-spawn-pre-register fault",
    },
    Case {
        label: "post-register",
        arm: |h| h.inject_post_register_fault.store(true, Ordering::SeqCst),
        consumed: |h| h.post_register_fault_consumed(),
        expected_fragment: "test-injected post-register fault",
    },
];

#[tokio::test]
async fn three_inject_points_each_yield_sandbox_build() {
    let names = ["fixture-pre", "fixture-mid", "fixture-post"];
    let mut builder = FixtureLockBuilder::new();
    for name in &names {
        builder = builder.add(FixtureSpec::new(name, "ready_only"));
    }
    let built = builder.build();
    let project_root = built.layout.project.path().to_path_buf();

    for (i, case) in CASES.iter().enumerate() {
        let plan = built.plans[i].clone();
        let broker = Broker::new(built.broker_acl.clone()).expect("Broker::new");
        let sup = PluginSupervisor::new(
            broker,
            SupervisorConfig::default(),
            ToolSchemaCatalog::empty_for_tests(),
        );
        let hooks = sup.test_hooks();

        assert!(
            !(case.consumed)(&hooks),
            "{}: consumed flag must start false",
            case.label
        );
        (case.arm)(&hooks);

        let paths = SpawnPaths {
            project_root: project_root.clone(),
            private_state_dir: project_root
                .join(".rafaello-plugin-data")
                .join(&plan.topic_id),
        };

        let result = sup.spawn(&plan, &paths).await;
        match result {
            Err(SpawnError::SandboxBuild { canonical, source }) => {
                assert_eq!(
                    canonical, plan.canonical,
                    "{}: canonical mismatch",
                    case.label
                );
                let msg = source.to_string();
                assert!(
                    msg.contains(case.expected_fragment),
                    "{}: source `{msg}` missing fragment `{}`",
                    case.label,
                    case.expected_fragment
                );
            }
            Err(other) => panic!(
                "{}: expected SpawnError::SandboxBuild, got {other:?}",
                case.label
            ),
            Ok(_) => panic!("{}: expected error, got Ok", case.label),
        }

        assert!(
            (case.consumed)(&hooks),
            "{}: consumed flag must be true after fault",
            case.label
        );
    }
}
