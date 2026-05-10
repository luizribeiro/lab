//! `Broker::register_frontend` rejects attach ids that are not present
//! in the ACL with `BrokerError::FrontendNotInAcl` (scope §B2, c14).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn register_frontend_unknown_attach_id_returns_not_in_acl() {
    let known = AttachId::new("known").expect("attach id");
    let unknown = AttachId::new("unknown").expect("attach id");

    let mut frontends = BTreeMap::new();
    frontends.insert(known.clone(), FrontendAcl::default());
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    assert!(broker.frontend_acl(&known).is_some());
    assert!(broker.frontend_acl(&unknown).is_none());

    assert!(matches!(
        broker.try_reserve_frontend_registration(&unknown),
        Err(BrokerError::FrontendNotInAcl(a)) if a == unknown
    ));

    let (peer, _rx) = fresh_peer();
    let err = broker
        .register_frontend(unknown.clone(), peer)
        .expect_err("unknown attach id is rejected");
    assert!(matches!(err, BrokerError::FrontendNotInAcl(a) if a == unknown));
}
