//! c16 §L1a — `hold_silent` mode adopts `RFL_BUS_FD`, runs
//! the fittings client serve loop, and holds the connection
//! WITHOUT sending `frontend.ready`. Self-exits via
//! `RFL_FIXTURE_MAX_LIFETIME`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::fixture_smoke::spawn_fixture_with_bus;

#[tokio::test(flavor = "multi_thread")]
async fn hold_silent_holds_connection_without_ready() {
    let mut smoke = spawn_fixture_with_bus("hold_silent", &[("RFL_FIXTURE_MAX_LIFETIME", "2")]);

    let early = tokio::time::timeout(Duration::from_millis(500), smoke.events.recv()).await;
    assert!(
        matches!(early, Err(_)),
        "hold_silent must NOT send frontend.ready (got {early:?})"
    );

    let status = tokio::time::timeout(Duration::from_secs(5), smoke.child.wait())
        .await
        .expect("max-lifetime self-exit timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(0), "self-timeout exits 0");
}
