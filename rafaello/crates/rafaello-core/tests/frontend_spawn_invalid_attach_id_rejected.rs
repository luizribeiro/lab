//! c18 §F3 Phase A — `FrontendSupervisor::spawn` rejects an
//! `attach_id` that fails [`AttachId::new`] grammar validation
//! with `FrontendSpawnError::InvalidPlan { reason: AttachIdInvalid }`.

mod common;
use common::frontend_test_kit::{
    baseline_plan, broker_with_attach, executable_entry, paths, KNOWN_ATTACH_ID,
};

use rafaello_core::error::{FrontendSpawnError, InvalidFrontendPlanReason};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test]
async fn spawn_with_uppercase_attach_id_returns_attach_id_invalid() {
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let (_dir, entry) = executable_entry();
    let bad_attach = "BAD-ID";
    let plan = baseline_plan(bad_attach, entry);

    let err = match supervisor.spawn(&plan, &paths()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        FrontendSpawnError::InvalidPlan {
            reason: InvalidFrontendPlanReason::AttachIdInvalid { attach_id },
        } => assert_eq!(attach_id, bad_attach),
        other => panic!("expected AttachIdInvalid, got {other:?}"),
    }
}
