//! c16 §L1a — `exit_immediately` mode exits 0 without sending
//! `frontend.ready` or adopting `RFL_BUS_FD`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::fixture_smoke::spawn_fixture_no_bus;

#[tokio::test(flavor = "multi_thread")]
async fn exit_immediately_exits_zero() {
    let mut child = spawn_fixture_no_bus("exit_immediately", &[], &[]);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("exit_immediately wait timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(0));
}
