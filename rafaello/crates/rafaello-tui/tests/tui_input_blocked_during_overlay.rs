//! c25 / scope §TUI1: while in overlay mode the input line is non-editable.
//! `input_blocked()` returns `true` for `ConfirmOverlay` and `false` for
//! `Normal`. Unhandled keys (e.g. text input characters) leave the overlay
//! intact and produce no published answer.

use crossterm::event::KeyCode;
use rafaello_tui::{handle_overlay_key, overlay_from_confirm_request, InputMode};
use serde_json::json;

#[test]
fn input_blocked_during_overlay() {
    let normal = InputMode::Normal;
    assert!(!normal.input_blocked());
    assert!(!normal.is_overlay());

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
    assert!(mode.input_blocked());
    assert!(mode.is_overlay());

    for key in [
        KeyCode::Char('x'),
        KeyCode::Char(' '),
        KeyCode::Backspace,
        KeyCode::Left,
    ] {
        let (next, env) = handle_overlay_key(&mode, key);
        assert_eq!(next, mode, "non-answer keys must not change the mode");
        assert!(env.is_none(), "non-answer keys must not publish");
    }
}
