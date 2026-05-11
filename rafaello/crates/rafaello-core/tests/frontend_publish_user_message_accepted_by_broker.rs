//! A frontend `bus.publish` for `frontend.tui.user_message` with a
//! ULID `request_id` is accepted by `handle_frontend_publish` — i.e. it
//! is not rejected by `PublishOutsideGrant` (granted) or
//! `MissingRequestId` (request_id supplied per §B0). Re-emit to the
//! canonical `core.session.user_message` is c18 territory; this test
//! only observes the `BrokerError` return value (pi-1 B-4).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::{Broker, JsonRpcId};

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_publish_user_message_accepted_by_broker() {
    let attach_id = AttachId::new("tui").expect("attach id");
    let topic = "frontend.tui.user_message";
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
        .expect("registration succeeds");

    let request_id = JsonRpcId::from(ulid::Ulid::new().to_string());
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"text": "hello"},
        "request_id": request_id,
    });
    broker
        .handle_frontend_publish(&attach_id, &params)
        .expect("frontend publish for granted topic with request_id is accepted");
}
