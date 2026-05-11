//! c21 / pi-1 H-4 — multibyte UTF-8 path round-trips correctly
//! (pi-1 M-3 / pi-2 M2-3 byte-offset slicing).

mod common;

use common::mock_provider_handle::{payload_path, MockProviderHandle};

#[tokio::test]
async fn handles_multibyte_utf8_path() {
    let mut handle = MockProviderHandle::launch().await;
    handle.publish_user_message("what's in données.txt");

    let event = handle.recv_event().await;
    assert_eq!(payload_path(&event), "données.txt");
}
