//! c18 §F3 Phase A — `Broker::try_reserve_frontend_registration`
//! failing on an attach id that is not in the broker ACL surfaces
//! as `FrontendSpawnError::InvalidPlan { reason: AttachIdNotInAcl }`.

mod common;
use common::frontend_test_kit::{
    baseline_plan, broker_with_attach, executable_entry, paths, KNOWN_ATTACH_ID,
};

use rafaello_core::broker_acl::AttachId;
use rafaello_core::error::{FrontendSpawnError, InvalidFrontendPlanReason};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test]
async fn spawn_with_unknown_attach_id_returns_attach_id_not_in_acl() {
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let (_dir, entry) = executable_entry();
    let unknown = "stranger";
    let plan = baseline_plan(unknown, entry);

    let err = match supervisor.spawn(&plan, &paths()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        FrontendSpawnError::InvalidPlan {
            reason: InvalidFrontendPlanReason::AttachIdNotInAcl { attach_id },
        } => assert_eq!(attach_id, AttachId::new(unknown).unwrap()),
        other => panic!("expected AttachIdNotInAcl, got {other:?}"),
    }
}
