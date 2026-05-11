//! c21 / pi-1 H-4 — trailing punctuation stripped from extracted path.

mod common;

use common::mock_provider_handle::{payload_path, MockProviderHandle};

#[tokio::test]
async fn strips_trailing_punctuation_from_path() {
    let mut handle = MockProviderHandle::launch().await;
    handle.publish_user_message("what's in README.md?");

    let event = handle.recv_event().await;
    assert_eq!(payload_path(&event), "README.md");
}
