//! Frontend publishing on `frontend.<own>` (only 2 segments) is rejected
//! as `PublishOnReservedNamespace`. So is publishing under another
//! frontend's attach id (scope §B4, c15).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

fn build_broker(attach_id: &AttachId) -> Broker {
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id.clone(),
        FrontendAcl {
            subscribe_patterns: Default::default(),
            auto_subscribes: Default::default(),
            publish_topics: ["frontend.ui.confirm_answer".to_string()]
                .into_iter()
                .collect(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    Broker::new(acl).expect("acl is well-formed")
}

#[test]
fn two_segment_frontend_topic_rejected() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let broker = build_broker(&attach_id);

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let bad = "frontend.ui";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad),
        "expected PublishOnReservedNamespace for {bad:?}, got {err:?}"
    );
}

#[test]
fn other_frontend_attach_id_rejected() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let broker = build_broker(&attach_id);

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let bad = "frontend.other.event";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad),
        "expected PublishOnReservedNamespace for {bad:?}, got {err:?}"
    );
}
