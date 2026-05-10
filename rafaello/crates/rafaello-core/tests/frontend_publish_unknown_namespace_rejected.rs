//! Frontend publishing on a top-level namespace outside
//! `{core, provider, plugin, frontend}` is rejected as
//! `UnknownNamespace` (scope §B4, c15).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn unknown_namespace_rejected() {
    let attach_id = AttachId::new("ui").expect("attach id");
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
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let bad = "evil.foo.bar";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::UnknownNamespace { ref topic, .. } if topic == bad),
        "expected UnknownNamespace, got {err:?}"
    );
}
