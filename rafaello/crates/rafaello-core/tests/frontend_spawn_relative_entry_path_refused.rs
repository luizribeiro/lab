//! c18 §F3 Phase A — `entry_absolute` that is not an absolute path
//! surfaces as `FrontendSpawnError::InvalidPlan { reason:
//! EntryNotAbsolute }`.

use std::path::PathBuf;

mod common;
use common::frontend_test_kit::{baseline_plan, broker_with_attach, paths, KNOWN_ATTACH_ID};

use rafaello_core::error::{FrontendSpawnError, InvalidFrontendPlanReason};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test]
async fn spawn_with_relative_entry_returns_entry_not_absolute() {
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let bad_entry = PathBuf::from("relative/bin/frontend");
    let plan = baseline_plan(KNOWN_ATTACH_ID, bad_entry.clone());

    let err = match supervisor.spawn(&plan, &paths()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        FrontendSpawnError::InvalidPlan {
            reason: InvalidFrontendPlanReason::EntryNotAbsolute { path },
        } => assert_eq!(path, bad_entry),
        other => panic!("expected EntryNotAbsolute, got {other:?}"),
    }
}
