//! c25 / scope §TUI1: `s` publishes `answer = "always_allow_session"`.

use crossterm::event::KeyCode;
use rafaello_tui::{handle_overlay_key, overlay_from_confirm_request, InputMode};
use serde_json::json;

#[test]
fn s_publishes_always_allow_session_answer() {
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

    let (next, env) = handle_overlay_key(&mode, KeyCode::Char('s'));
    assert_eq!(next, InputMode::Normal);
    let env = env.expect("answer published");
    assert_eq!(
        env.payload,
        json!({ "request_id": "CID", "answer": "always_allow_session" })
    );
}
