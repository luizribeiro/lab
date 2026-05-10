//! c23 §H2 — `FixtureLockBuilder` builds a one-plugin lock that
//! survives `validate::lock` and `compile_plugin` cleanly.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec};

#[test]
fn one_plugin_lock_round_trips() {
    let built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("alpha", "respond_peer_call"))
        .build();

    assert_eq!(built.plans.len(), 1);
    let plan = &built.plans[0];
    assert_eq!(plan.canonical.name(), "alpha");
    assert!(plan.entry_absolute.is_absolute());
    assert!(
        plan.entry_absolute.ends_with("bin/fixture"),
        "entry_absolute should end with bin/fixture, got {:?}",
        plan.entry_absolute
    );
    assert!(
        built.broker_acl.plugins.contains_key(&plan.canonical),
        "broker ACL should list the plugin"
    );
    assert_eq!(
        built.broker_acl.plugins[&plan.canonical].topic_id, plan.topic_id,
        "ACL topic_id matches plan topic_id"
    );
}
