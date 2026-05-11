//! c25 / scope §TUI1: `y`, `a`, and `Enter` keys all publish a
//! `frontend.tui.confirm_answer` with `answer = "allow"` and an envelope
//! `request_id` that is fresh (distinct from the payload's `request_id`),
//! envelope `in_reply_to = [confirm_id]`.

use crossterm::event::KeyCode;
use fittings_core::message::JsonRpcId;
use rafaello_tui::{
    handle_overlay_key, overlay_from_confirm_request, InputMode, CONFIRM_ANSWER_TOPIC,
};
use serde_json::json;

fn fresh_overlay(id: &str) -> InputMode {
    overlay_from_confirm_request(
        &json!({
            "request_id": id,
            "summary": "s",
            "details": {},
            "ttl_seconds": 60_u64,
        }),
        0,
    )
    .unwrap()
}

#[test]
fn y_a_enter_publish_allow_answer() {
    let mode = fresh_overlay("01HZ_CONFIRM");
    let confirm_id = JsonRpcId::String("01HZ_CONFIRM".to_string());

    for key in [KeyCode::Char('y'), KeyCode::Char('a'), KeyCode::Enter] {
        let (next, env) = handle_overlay_key(&mode, key);
        assert_eq!(next, InputMode::Normal);
        let env = env.expect("answer published");
        assert_eq!(env.topic, CONFIRM_ANSWER_TOPIC);
        assert_eq!(env.in_reply_to, vec![confirm_id.clone()]);
        assert_ne!(env.request_id, confirm_id, "envelope id must be fresh");
        assert_eq!(
            env.payload,
            json!({ "request_id": "01HZ_CONFIRM", "answer": "allow" })
        );
    }
}
