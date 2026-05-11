//! c23 / scope §TP2: missing file → `{ok: false, error}`.

mod common;

use common::read_file_tool_handle::{payload_error, payload_ok, LaunchOpts, ReadFileToolHandle};

#[tokio::test]
async fn errors_for_missing_file() {
    let project_root = tempfile::tempdir().expect("project tempdir");

    let mut handle = ReadFileToolHandle::launch(LaunchOpts {
        project_root: project_root.path().to_path_buf(),
        bypass_guard: false,
        sandbox_read_dirs: None,
    })
    .await;

    let req_id = handle.publish_tool_request("does-not-exist.md");
    let event = handle.recv_event().await;

    assert!(
        !payload_ok(&event),
        "expected ok=false, got {:?}",
        event.payload
    );
    assert!(
        !payload_error(&event).is_empty(),
        "error field must be populated"
    );
    assert_eq!(event.in_reply_to.as_deref(), Some(&[req_id][..]));
}
