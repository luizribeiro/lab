//! c20 acceptance: when the reaper has already observed the child's
//! clean exit, `shutdown` returns without invoking SIGTERM/SIGKILL.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::frontend_test_kit::{broker_with_attach, fixture_plan, live_paths, KNOWN_ATTACH_ID};
use rafaello_core::error::ReaperOutcome;
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_handle_shutdown_skips_kill_on_exited() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let plan = fixture_plan(
        KNOWN_ATTACH_ID,
        "signal_ready",
        &[("RFL_FIXTURE_MAX_LIFETIME", "1")],
    );
    let paths = live_paths(&tmp);

    let mut handle = supervisor.spawn(&plan, &paths).await.expect("spawn ok");
    let outcome = tokio::time::timeout(Duration::from_secs(10), handle.wait())
        .await
        .expect("wait timed out");
    assert!(matches!(&*outcome, ReaperOutcome::Exited(_)));
    let report = handle.shutdown().await;
    assert!(!report.used_sigterm, "should skip SIGTERM on Exited");
    assert!(!report.used_sigkill, "should skip SIGKILL on Exited");
    assert!(report.exit_status.is_some(), "exit_status populated");
}
