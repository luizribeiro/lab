//! c16 §L1a — `signal_ready` mode adopts `RFL_BUS_FD`, peer-calls
//! `frontend.ready`, then sleeps until SIGTERM / `RFL_FIXTURE_MAX_LIFETIME`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::fixture_smoke::{spawn_fixture_with_bus, wait_for_method};

#[tokio::test(flavor = "multi_thread")]
async fn signal_ready_emits_frontend_ready_then_self_exits() {
    let mut smoke = spawn_fixture_with_bus("signal_ready", &[("RFL_FIXTURE_MAX_LIFETIME", "2")]);

    let recorded =
        wait_for_method(&mut smoke.events, "frontend.ready", Duration::from_secs(5)).await;
    assert!(
        !recorded.is_notification,
        "frontend.ready must be peer-call"
    );

    let status = tokio::time::timeout(Duration::from_secs(5), smoke.child.wait())
        .await
        .expect("max-lifetime self-exit timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(0), "self-timeout exits 0");
}
