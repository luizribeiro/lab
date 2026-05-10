//! c18 §F3 Phase A — `entry_absolute` that exists but lacks the
//! executable bit surfaces as `FrontendSpawnError::InvalidPlan
//! { reason: EntryNotExecutable }`.

use std::os::unix::fs::PermissionsExt;

mod common;
use common::frontend_test_kit::{baseline_plan, broker_with_attach, paths, KNOWN_ATTACH_ID};

use rafaello_core::error::{FrontendSpawnError, InvalidFrontendPlanReason};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test]
async fn spawn_with_non_executable_entry_returns_entry_not_executable() {
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());

    let dir = tempfile::tempdir().expect("tempdir");
    let entry = dir.path().join("frontend-bin");
    std::fs::write(&entry, b"#!/bin/sh\nexit 0\n").expect("write entry");
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o644)).expect("chmod 0644");

    let plan = baseline_plan(KNOWN_ATTACH_ID, entry.clone());

    let err = match supervisor.spawn(&plan, &paths()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        FrontendSpawnError::InvalidPlan {
            reason: InvalidFrontendPlanReason::EntryNotExecutable { path },
        } => assert_eq!(path, entry),
        other => panic!("expected EntryNotExecutable, got {other:?}"),
    }
}
