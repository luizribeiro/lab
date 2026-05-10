//! c18 §F3 Phase A — a reserved env name in `env.set` surfaces
//! as `FrontendSpawnError::InvalidPlan { reason: ReservedEnvName }`.

mod common;
use common::frontend_test_kit::{
    baseline_plan, broker_with_attach, executable_entry, paths, KNOWN_ATTACH_ID,
};

use rafaello_core::error::{FrontendSpawnError, InvalidFrontendPlanReason};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test]
async fn spawn_with_reserved_env_in_set_returns_reserved_env_name() {
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let (_dir, entry) = executable_entry();
    let mut plan = baseline_plan(KNOWN_ATTACH_ID, entry);
    plan.env
        .set
        .insert("RFL_TOPIC_ID".to_string(), "x".to_string());

    let err = match supervisor.spawn(&plan, &paths()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        FrontendSpawnError::InvalidPlan {
            reason: InvalidFrontendPlanReason::ReservedEnvName { var },
        } => assert_eq!(var, "RFL_TOPIC_ID"),
        other => panic!("expected ReservedEnvName, got {other:?}"),
    }
}
