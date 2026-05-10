//! Registering the same attach id twice while the first guard is still
//! alive returns `FrontendAlreadyRegistered` (scope §B2, c14).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn second_register_for_live_attach_id_returns_already_registered() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(attach_id.clone(), FrontendAcl::default());
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer_a, _rx_a) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer_a)
        .expect("first registration succeeds");

    let (peer_b, _rx_b) = fresh_peer();
    let err = broker
        .register_frontend(attach_id.clone(), peer_b)
        .expect_err("second registration for live attach id is rejected");
    assert!(matches!(err, BrokerError::FrontendAlreadyRegistered(a) if a == attach_id));
}
