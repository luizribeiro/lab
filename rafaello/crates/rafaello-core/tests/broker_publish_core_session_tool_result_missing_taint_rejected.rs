//! Symmetric defence-in-depth for `core.session.tool_result`:
//! `publish_core` (the taint-less wrapper) is rejected with
//! `InvalidTaint{Missing}` (scope §B8).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::error::{BrokerError, TaintReason};

#[test]
fn publish_core_tool_result_missing_taint_rejected() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let err = broker
        .publish_core("core.session.tool_result", serde_json::json!({}))
        .expect_err("missing taint must be rejected");

    match err {
        BrokerError::InvalidTaint {
            reason: TaintReason::Missing,
            topic,
            ..
        } => {
            assert_eq!(topic, "core.session.tool_result");
        }
        other => panic!("expected InvalidTaint{{Missing}}, got {other:?}"),
    }
}
