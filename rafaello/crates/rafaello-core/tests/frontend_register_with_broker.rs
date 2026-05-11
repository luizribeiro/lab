//! Stand-alone positive happy-path for frontend registration: a frontend
//! whose `attach_id` is in the ACL registers cleanly, the guard drops
//! cleanly, and the slot is freed for re-registration (m3 retro §5.9
//! file-granularity gap closer).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_register_with_broker() {
    let attach_id = AttachId::new("tui").expect("attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id.clone(),
        FrontendAcl {
            subscribe_patterns: Default::default(),
            auto_subscribes: Default::default(),
            publish_topics: Default::default(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let (peer2, _rx2) = fresh_peer();
    let dup_err = broker
        .register_frontend(attach_id.clone(), peer2)
        .expect_err("re-registration while guard is alive must be rejected");
    assert!(
        matches!(dup_err, BrokerError::FrontendAlreadyRegistered(ref a) if a == &attach_id),
        "expected FrontendAlreadyRegistered, got {dup_err:?}"
    );

    drop(guard);

    let (peer3, _rx3) = fresh_peer();
    let _guard2 = broker
        .register_frontend(attach_id.clone(), peer3)
        .expect("registration succeeds again after guard drop");
}
