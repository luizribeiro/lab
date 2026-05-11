//! A frontend `bus.publish` for `frontend.tui.evil_topic` (not in the
//! TUI principal's grant set) is rejected with `PublishOutsideGrant`
//! (scope §CT4): the c12 ACL extension grants only `user_message`,
//! `confirm_answer`, and `slash_command` — anything else must be
//! refused.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_publish_unknown_topic_rejected() {
    let attach_id = AttachId::new("tui").expect("attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id.clone(),
        FrontendAcl {
            subscribe_patterns: Default::default(),
            auto_subscribes: Default::default(),
            publish_topics: [
                "frontend.tui.user_message".to_string(),
                "frontend.tui.confirm_answer".to_string(),
                "frontend.tui.slash_command".to_string(),
            ]
            .into_iter()
            .collect(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let bad = "frontend.tui.evil_topic";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOutsideGrant { ref topic, .. } if topic == bad),
        "expected PublishOutsideGrant, got {err:?}"
    );
}
