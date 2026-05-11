//! A frontend `bus.publish` for `frontend.tui.confirm_answer` with a
//! ULID `request_id` is accepted by `handle_frontend_publish` — i.e.
//! it is not rejected by `PublishOutsideGrant`. Mirrors the c01-style
//! shape (scope §CT4).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::{Broker, JsonRpcId};

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_publish_confirm_answer_accepted_by_broker() {
    let attach_id = AttachId::new("tui").expect("attach id");
    let topic = "frontend.tui.confirm_answer";
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

    let request_id = JsonRpcId::from(ulid::Ulid::new().to_string());
    let in_reply_to = JsonRpcId::from(ulid::Ulid::new().to_string());
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"answer": "yes"},
        "in_reply_to": [in_reply_to],
        "request_id": request_id,
    });
    broker
        .handle_frontend_publish(&attach_id, &params)
        .expect("frontend publish for granted confirm_answer is accepted");
}
