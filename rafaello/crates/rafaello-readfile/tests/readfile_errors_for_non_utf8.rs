//! c23 / scope §TP2: non-utf8 file body → `{ok: false, error}` (m4 cut
//! is utf8-only).

mod common;

use common::read_file_tool_handle::{payload_error, payload_ok, LaunchOpts, ReadFileToolHandle};

#[tokio::test]
async fn errors_for_non_utf8() {
    let project_root = tempfile::tempdir().expect("project tempdir");
    let bytes: [u8; 4] = [0xff, 0xfe, 0xfd, 0xfc];
    std::fs::write(project_root.path().join("binary.bin"), bytes).expect("write binary");

    let mut handle = ReadFileToolHandle::launch(LaunchOpts {
        project_root: project_root.path().to_path_buf(),
        bypass_guard: false,
        sandbox_read_dirs: None,
    })
    .await;

    let req_id = handle.publish_tool_request("binary.bin");
    let event = handle.recv_event().await;

    assert!(
        !payload_ok(&event),
        "expected ok=false, got {:?}",
        event.payload
    );
    assert!(
        payload_error(&event).contains("utf-8"),
        "error must mention utf-8, got {:?}",
        event.payload
    );
    assert_eq!(event.in_reply_to.as_deref(), Some(&[req_id][..]));
}
