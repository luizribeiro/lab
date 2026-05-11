//! c23 / scope §TP3 + pi-1 H-3: sandbox-level negative. The bypass
//! env (`RFL_READFILE_TEST_BYPASS_GUARD=1`) makes the plugin skip its
//! own ancestor check and call `std::fs::read` on the raw path. With
//! `read_dirs` restricted to project A, a read against a path in
//! tempdir B is denied by the lockin sandbox; the surfaced
//! `tool_result.error` is the rendered `io::Error` (kind =
//! `PermissionDenied`).

#![cfg(target_os = "linux")]

mod common;

use common::read_file_tool_handle::{payload_error, payload_ok, LaunchOpts, ReadFileToolHandle};

#[tokio::test(flavor = "multi_thread")]
async fn lockin_denies_outside_grant() {
    let project_a = tempfile::tempdir().expect("project A tempdir");
    let project_b = tempfile::tempdir().expect("project B tempdir");
    let outside = project_b.path().join("secret.txt");
    std::fs::write(&outside, "shhh").expect("write outside");

    let mut handle = ReadFileToolHandle::launch(LaunchOpts {
        project_root: project_a.path().to_path_buf(),
        bypass_guard: true,
        sandbox_read_dirs: Some(vec![project_a.path().to_path_buf()]),
    })
    .await;

    let req_id = handle.publish_tool_request(outside.to_str().expect("utf-8 path"));
    let event = handle.recv_event().await;

    assert!(
        !payload_ok(&event),
        "expected ok=false from sandbox denial, got {:?}",
        event.payload
    );
    let err = payload_error(&event);
    assert_ne!(
        err, "path denied",
        "denial must come from the lockin sandbox, not the in-plugin ancestor check (which was bypassed)"
    );
    // The lockin sandbox renders denials as one of PermissionDenied (EPERM/EACCES)
    // or NotFound (ENOENT); accept any since syd's choice varies by syscall family.
    // Matches supervisor_lockin_denies_outside_grant_read.rs's tolerance.
    let permitted = err.contains("permission denied")
        || err.contains("entity not found")
        || err.contains("not found")
        || err.contains("No such file");
    assert!(
        permitted,
        "error must surface a sandbox denial (EPERM/EACCES/ENOENT), got {err:?}"
    );
    assert_eq!(event.in_reply_to.as_deref(), Some(&[req_id][..]));
}
