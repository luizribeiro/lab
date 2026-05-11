//! Frontend publishing `frontend.tui.confirm_answer` without
//! `in_reply_to` is rejected as `InvalidInReplyTo { Missing }` —
//! scope §CT3 mandatory-`in_reply_to` extension.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::error::{InReplyToReason, Publisher};
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_confirm_answer_missing_in_reply_to_rejected() {
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

    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "request_id": JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                publisher: Publisher::Frontend(_),
                reason: InReplyToReason::Missing,
                ..
            }
        ),
        "expected InvalidInReplyTo{{Frontend, Missing}}, got {err:?}"
    );
}
