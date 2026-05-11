//! c30 / scope §TP3 negative: omitting `args.to` returns
//! `{ok: false, error: "missing 'to' field"}` and writes nothing
//! to the log.

use rafaello_mailcat::{handle_tool_request, LOG_FILE_NAME};
use serde_json::json;

#[test]
fn returns_error_on_missing_to_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let payload = json!({
        "tool": "send-mail",
        "args": {"subject": "hi", "body": "hello"},
    });

    let response = handle_tool_request(&payload, dir.path());

    assert_eq!(
        response,
        json!({"ok": false, "error": "missing 'to' field"})
    );

    let log_path = dir.path().join(LOG_FILE_NAME);
    assert!(
        !log_path.exists(),
        "log file must not be created when request is rejected"
    );
}

#[test]
fn returns_error_when_args_missing_entirely() {
    let dir = tempfile::tempdir().expect("tempdir");
    let payload = json!({"tool": "send-mail"});

    let response = handle_tool_request(&payload, dir.path());

    assert_eq!(
        response,
        json!({"ok": false, "error": "missing 'to' field"})
    );
}
