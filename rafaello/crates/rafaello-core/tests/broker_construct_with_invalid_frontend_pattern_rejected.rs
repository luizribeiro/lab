//! `Broker::new` runs §B10 defence-in-depth grammar revalidation on
//! every frontend ACL entry's `subscribe_patterns` and `auto_subscribes`
//! and rejects hand-built ACLs that bypassed `broker_acl::compile`
//! (scope §B2 + §B10, c14).

use std::collections::{BTreeMap, BTreeSet};

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

#[test]
fn invalid_frontend_subscribe_pattern_rejected_at_construction() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let mut subscribe_patterns = BTreeSet::new();
    subscribe_patterns.insert("**".to_string());

    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id,
        FrontendAcl {
            subscribe_patterns,
            auto_subscribes: BTreeSet::new(),
            publish_topics: BTreeSet::new(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let err = Broker::new(acl).expect_err("standalone `**` is invalid grammar");
    assert!(
        matches!(err, BrokerError::InvalidPattern { .. }),
        "expected InvalidPattern, got {err:?}",
    );
}

#[test]
fn invalid_frontend_auto_subscribe_pattern_rejected_at_construction() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let mut auto_subscribes = BTreeSet::new();
    auto_subscribes.insert("plugin.**.foo".to_string());

    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id,
        FrontendAcl {
            subscribe_patterns: BTreeSet::new(),
            auto_subscribes,
            publish_topics: BTreeSet::new(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let err = Broker::new(acl).expect_err("non-final `**` is invalid grammar");
    assert!(
        matches!(err, BrokerError::InvalidPattern { .. }),
        "expected InvalidPattern, got {err:?}",
    );
}
