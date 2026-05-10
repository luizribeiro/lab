//! c16 §L1a — `signal_ready_then_exit_n` mode signals
//! `frontend.ready`, sleeps 200 ms, then exits with the code
//! from `RFL_FIXTURE_EXIT_CODE` (default 7).

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::fixture_smoke::{spawn_fixture_with_bus, wait_for_method};

#[tokio::test(flavor = "multi_thread")]
async fn signal_ready_then_exit_n_exits_with_configured_code() {
    let mut smoke = spawn_fixture_with_bus(
        "signal_ready_then_exit_n",
        &[
            ("RFL_FIXTURE_MAX_LIFETIME", "5"),
            ("RFL_FIXTURE_EXIT_CODE", "9"),
        ],
    );

    wait_for_method(&mut smoke.events, "frontend.ready", Duration::from_secs(5)).await;

    let status = tokio::time::timeout(Duration::from_secs(5), smoke.child.wait())
        .await
        .expect("post-ready exit timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(9), "exits with RFL_FIXTURE_EXIT_CODE");
}

#[tokio::test(flavor = "multi_thread")]
async fn signal_ready_then_exit_n_default_exit_code_is_seven() {
    let mut smoke = spawn_fixture_with_bus(
        "signal_ready_then_exit_n",
        &[("RFL_FIXTURE_MAX_LIFETIME", "5")],
    );

    wait_for_method(&mut smoke.events, "frontend.ready", Duration::from_secs(5)).await;

    let status = tokio::time::timeout(Duration::from_secs(5), smoke.child.wait())
        .await
        .expect("post-ready exit timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(7), "default exit code is 7");
}
