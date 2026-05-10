//! c20 acceptance: `wait_ready` resolves once the child invokes
//! `frontend.ready` over the bus.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::frontend_test_kit::{broker_with_attach, fixture_plan, live_paths, KNOWN_ATTACH_ID};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_handle_wait_ready_resolves_on_signal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let plan = fixture_plan(
        KNOWN_ATTACH_ID,
        "signal_ready",
        &[("RFL_FIXTURE_MAX_LIFETIME", "5")],
    );
    let paths = live_paths(&tmp);

    let mut handle = supervisor.spawn(&plan, &paths).await.expect("spawn ok");
    tokio::time::timeout(Duration::from_secs(5), handle.wait_ready())
        .await
        .expect("wait_ready timed out")
        .expect("wait_ready should resolve Ok");
    assert!(handle.has_signalled_ready());
}
