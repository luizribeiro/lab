//! c25 / scope §TUI1: when a `core.session.confirm_reply` event arrives for
//! the active overlay's `confirm_id`, the overlay exits (back to `Normal`).
//! A reply for a different id leaves the overlay intact.

use rafaello_tui::{handle_confirm_reply, overlay_from_confirm_request, InputMode};
use serde_json::json;

fn overlay(id: &str) -> InputMode {
    overlay_from_confirm_request(
        &json!({
            "request_id": id,
            "summary": "",
            "details": {},
            "ttl_seconds": 60_u64,
        }),
        0,
    )
    .unwrap()
}

#[test]
fn overlay_exits_on_matching_confirm_reply() {
    let mode = overlay("CID");
    let after = handle_confirm_reply(&mode, &json!({ "request_id": "CID", "result": "allow" }));
    assert_eq!(after, InputMode::Normal);
}

#[test]
fn overlay_preserved_on_mismatched_confirm_reply() {
    let mode = overlay("CID");
    let after = handle_confirm_reply(&mode, &json!({ "request_id": "OTHER", "result": "allow" }));
    assert_eq!(after, mode);
}

#[test]
fn normal_mode_unaffected_by_confirm_reply() {
    let mode = InputMode::Normal;
    let after = handle_confirm_reply(&mode, &json!({ "request_id": "CID" }));
    assert_eq!(after, InputMode::Normal);
}
