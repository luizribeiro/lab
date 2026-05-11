//! c25 / scope §TUI1 (pi-1 M-4): the overlay is TUI-internal UI and is **not**
//! a persisted entry kind. Observing `core.session.confirm_request` enters the
//! overlay but produces no `core.session.entry.finalized` publish action. The
//! only outbound publish on user input is `frontend.tui.confirm_answer`.

use crossterm::event::KeyCode;
use fittings_core::message::JsonRpcId;
use rafaello_tui::{
    build_confirm_answer, handle_overlay_key, overlay_from_confirm_request, Answer,
    CONFIRM_ANSWER_TOPIC,
};
use serde_json::json;

#[test]
fn overlay_does_not_persist_entry_for_confirm_request() {
    let payload = json!({
        "request_id": "CID",
        "what": "tool_call",
        "summary": "s",
        "details": {},
        "ttl_seconds": 60_u64,
    });

    let mode = overlay_from_confirm_request(&payload, 0).expect("overlay built");

    let (_next, env_on_allow) = handle_overlay_key(&mode, KeyCode::Char('y'));
    let env_on_allow = env_on_allow.expect("publishes on key");
    assert_eq!(env_on_allow.topic, CONFIRM_ANSWER_TOPIC);
    assert_ne!(env_on_allow.topic, "core.session.entry.finalized");

    let env_direct = build_confirm_answer(&JsonRpcId::String("CID".to_string()), Answer::Deny);
    assert_eq!(env_direct.topic, CONFIRM_ANSWER_TOPIC);
    assert_ne!(env_direct.topic, "core.session.entry.finalized");
}
