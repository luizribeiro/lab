//! c23 / scope §TP2 + pi-1 H-3 (plugin-level negative): a path that
//! resolves outside `RFL_PROJECT_ROOT` is rejected by the in-plugin
//! ancestor check with `{ok: false, error: "path denied"}`.

mod common;

use common::read_file_tool_handle::{payload_error, payload_ok, LaunchOpts, ReadFileToolHandle};

#[tokio::test]
async fn errors_for_outside_project_root() {
    let project_root = tempfile::tempdir().expect("project tempdir");
    let other = tempfile::tempdir().expect("other tempdir");
    let outside = other.path().join("secret.txt");
    std::fs::write(&outside, "shhh").expect("write outside");

    let mut handle = ReadFileToolHandle::launch(LaunchOpts {
        project_root: project_root.path().to_path_buf(),
        bypass_guard: false,
        sandbox_read_dirs: None,
    })
    .await;

    let req_id = handle.publish_tool_request(outside.to_str().expect("utf-8 path"));
    let event = handle.recv_event().await;

    assert!(
        !payload_ok(&event),
        "expected ok=false, got {:?}",
        event.payload
    );
    assert_eq!(
        payload_error(&event),
        "path denied",
        "ancestor check must reject as 'path denied', got {:?}",
        event.payload
    );
    assert_eq!(event.in_reply_to.as_deref(), Some(&[req_id][..]));
}
