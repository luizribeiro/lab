//! c20 acceptance: `wait_ready` errors with `SenderDropped` when
//! the child exits before invoking `frontend.ready`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::frontend_test_kit::{broker_with_attach, fixture_plan, live_paths, KNOWN_ATTACH_ID};
use rafaello_core::frontend::{FrontendConfig, FrontendReadyError, FrontendSupervisor};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_handle_wait_ready_errors_on_child_exit_before_signal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let plan = fixture_plan(KNOWN_ATTACH_ID, "exit_immediately", &[]);
    let paths = live_paths(&tmp);

    let mut handle = supervisor.spawn(&plan, &paths).await.expect("spawn ok");
    let result = tokio::time::timeout(Duration::from_secs(5), handle.wait_ready())
        .await
        .expect("wait_ready timed out");
    match result {
        Err(FrontendReadyError::SenderDropped) => {}
        other => panic!("expected SenderDropped, got {:?}", other),
    }
}
