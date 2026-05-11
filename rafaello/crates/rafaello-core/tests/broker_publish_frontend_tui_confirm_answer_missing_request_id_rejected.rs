//! Frontend publishing `frontend.tui.confirm_answer` with
//! `request_id: None` is rejected as `MissingRequestId` — scope §CT2
//! suffix-list extension applied inside `handle_frontend_publish`.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::error::Publisher;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_confirm_answer_missing_request_id_rejected() {
    let attach_id = AttachId::new("tui").expect("attach id");
    let topic = "frontend.tui.confirm_answer";
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id.clone(),
        FrontendAcl {
            subscribe_patterns: Default::default(),
            auto_subscribes: Default::default(),
            publish_topics: [topic.to_string()].into_iter().collect(),
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
        .expect("registered");

    let params = serde_json::json!({"topic": topic, "payload": {}});
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::MissingRequestId {
                publisher: Publisher::Frontend(_),
                ..
            }
        ),
        "expected MissingRequestId{{Frontend}}, got {err:?}"
    );
}
