//! `Broker::new` runs §B10 defence-in-depth grammar revalidation on
//! every frontend ACL entry's `publish_topics` and rejects hand-built
//! ACLs that bypassed `broker_acl::compile` (scope §B2 + §B10, c14).

use std::collections::{BTreeMap, BTreeSet};

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

#[test]
fn invalid_frontend_publish_topic_rejected_at_construction() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let mut publish_topics = BTreeSet::new();
    publish_topics.insert("frontend.id.UPPER".to_string());

    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id,
        FrontendAcl {
            subscribe_patterns: BTreeSet::new(),
            auto_subscribes: BTreeSet::new(),
            publish_topics,
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let err = Broker::new(acl).expect_err("uppercase topic segment is invalid");
    assert!(
        matches!(err, BrokerError::InvalidTopic { .. }),
        "expected InvalidTopic, got {err:?}",
    );
}
