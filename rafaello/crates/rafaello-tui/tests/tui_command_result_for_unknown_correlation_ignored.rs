//! c19 / scope §SL4: a `core.session.command_result` whose
//! `in_reply_to[0]` does not match any pending slash request is
//! silently ignored.

use rafaello_tui::command_result::{paint_frame, CommandResultState, TOPIC_COMMAND_RESULT};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::json;

#[test]
fn unknown_correlation_is_silently_ignored() {
    let mut state = CommandResultState::new();
    state.note_pending("KNOWN");
    let event = json!({
        "topic": TOPIC_COMMAND_RESULT,
        "in_reply_to": ["STRANGER"],
        "payload": { "message": "should be dropped" },
    });
    assert!(!state.ingest_event(&event));
    assert!(state.callouts().is_empty());

    let mut term = Terminal::new(TestBackend::new(40, 4)).unwrap();
    let entries = vec!["entry".to_string()];
    paint_frame(&mut term, &entries, state.callouts()).unwrap();

    let buf = term.backend().buffer();
    let mut all = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            all.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(!all.contains("should be dropped"));
}
