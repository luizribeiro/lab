//! c25 / scope §TUI1: `Esc` publishes `answer = "deny"` (matches `n` / `d`).

use crossterm::event::KeyCode;
use rafaello_tui::{handle_overlay_key, overlay_from_confirm_request, InputMode};
use serde_json::json;

#[test]
fn esc_publishes_deny_answer() {
    let mode = overlay_from_confirm_request(
        &json!({
            "request_id": "CID",
            "summary": "",
            "details": {},
            "ttl_seconds": 60_u64,
        }),
        0,
    )
    .unwrap();

    let (next, env) = handle_overlay_key(&mode, KeyCode::Esc);
    assert_eq!(next, InputMode::Normal);
    let env = env.expect("answer published");
    assert_eq!(
        env.payload,
        json!({ "request_id": "CID", "answer": "deny" })
    );
}
