//! c20 acceptance: when the child exits cleanly, the reaper task
//! publishes `ReaperOutcome::Exited(status)` with `status.success()`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::frontend_test_kit::{broker_with_attach, fixture_plan, live_paths, KNOWN_ATTACH_ID};
use rafaello_core::error::ReaperOutcome;
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_reaper_publishes_exited_on_clean_exit() {
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
        .expect("reaper outcome timed out");
    match &*outcome {
        ReaperOutcome::Exited(status) => assert!(status.success(), "child should exit 0"),
        other => panic!("expected Exited, got {:?}", other),
    }
}
