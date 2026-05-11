//! c30 / scope §TP3 happy path: `handle_tool_request` appends the
//! request payload to `mailcat.log` under the per-plugin private
//! state dir and returns `{ok: true}`.

use rafaello_mailcat::{handle_tool_request, LOG_FILE_NAME};
use serde_json::json;

#[test]
fn appends_to_log_on_tool_request() {
    let dir = tempfile::tempdir().expect("tempdir");
    let payload = json!({
        "tool": "send-mail",
        "args": {"to": "alice@example.com", "subject": "hi", "body": "hello"},
    });

    let response = handle_tool_request(&payload, dir.path());

    assert_eq!(response, json!({"ok": true}));

    let log_path = dir.path().join(LOG_FILE_NAME);
    let contents = std::fs::read_to_string(&log_path).expect("read log");
    assert!(
        contents.contains("alice@example.com"),
        "log missing 'to' field; got: {contents:?}"
    );
    assert!(
        contents.ends_with('\n'),
        "log must be newline-terminated; got: {contents:?}"
    );

    let second = json!({"tool": "send-mail", "args": {"to": "bob@example.com"}});
    let response2 = handle_tool_request(&second, dir.path());
    assert_eq!(response2, json!({"ok": true}));
    let combined = std::fs::read_to_string(&log_path).expect("read log");
    assert_eq!(
        combined.lines().count(),
        2,
        "expected two appended lines, got: {combined:?}"
    );
    assert!(combined.contains("bob@example.com"));
}
