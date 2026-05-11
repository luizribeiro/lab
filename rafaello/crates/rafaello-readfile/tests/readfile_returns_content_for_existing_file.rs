//! c23 / scope §TP2 happy path: `rfl-readfile` reads a file under
//! `RFL_PROJECT_ROOT` and publishes the canonical
//! `{ok: true, content}` `tool_result` wire shape.

mod common;

use common::read_file_tool_handle::{
    payload_content, payload_ok, LaunchOpts, ReadFileToolHandle, TOPIC_ID,
};

#[tokio::test]
async fn returns_content_for_existing_file() {
    let project_root = tempfile::tempdir().expect("project tempdir");
    let body = "m4 demo readme\n";
    std::fs::write(project_root.path().join("README.md"), body).expect("write README");

    let mut handle = ReadFileToolHandle::launch(LaunchOpts {
        project_root: project_root.path().to_path_buf(),
        bypass_guard: false,
        sandbox_read_dirs: None,
    })
    .await;

    let req_id = handle.publish_tool_request("README.md");
    let event = handle.recv_event().await;

    assert_eq!(event.topic, format!("plugin.{TOPIC_ID}.tool_result"));
    assert!(
        payload_ok(&event),
        "expected ok=true, got {:?}",
        event.payload
    );
    assert_eq!(payload_content(&event), body);
    assert!(
        event.request_id.is_some(),
        "tool_result must carry fresh request_id"
    );
    assert_ne!(
        event.request_id.as_ref(),
        Some(&req_id),
        "tool_result request_id must be distinct from the inbound tool_request id"
    );
    assert_eq!(event.in_reply_to.as_deref(), Some(&[req_id][..]));
}
