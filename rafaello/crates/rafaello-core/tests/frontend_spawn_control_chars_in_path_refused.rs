//! c18 §F3 Phase A — control characters in `entry_absolute`
//! surface as `FrontendSpawnError::InvalidPlan { reason:
//! ControlCharsInPath }` (m2 §SP4 pattern).

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

mod common;
use common::frontend_test_kit::{baseline_plan, broker_with_attach, paths, KNOWN_ATTACH_ID};

use rafaello_core::error::{FrontendSpawnError, InvalidFrontendPlanReason};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test]
async fn spawn_with_control_chars_returns_control_chars_in_path() {
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let bad_entry = PathBuf::from(OsString::from_vec(b"/tmp/bin/\x01frontend".to_vec()));
    let plan = baseline_plan(KNOWN_ATTACH_ID, bad_entry.clone());

    let err = match supervisor.spawn(&plan, &paths()).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    match err {
        FrontendSpawnError::InvalidPlan {
            reason: InvalidFrontendPlanReason::ControlCharsInPath { path },
        } => assert_eq!(path, bad_entry),
        other => panic!("expected ControlCharsInPath, got {other:?}"),
    }
}
