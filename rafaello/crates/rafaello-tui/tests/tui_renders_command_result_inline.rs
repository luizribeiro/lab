//! c19 / scope §SL4: a matching `core.session.command_result` is
//! rendered inline as a transient callout above the input, distinct
//! from the conversation entry list.

use rafaello_tui::command_result::{paint_frame, CommandResultState, TOPIC_COMMAND_RESULT};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::json;

fn rows(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn renders_matched_command_result_in_callout_region() {
    let mut state = CommandResultState::new();
    state.note_pending("SLASH_REQ_1");

    let entries = vec!["entry one".to_string(), "entry two".to_string()];
    let event = json!({
        "topic": TOPIC_COMMAND_RESULT,
        "in_reply_to": ["SLASH_REQ_1"],
        "payload": { "ok": true, "kind": "grant", "message": "grant added: tool_a", "details": {} },
    });

    assert!(state.ingest_event(&event));
    assert_eq!(state.callouts().len(), 1);
    assert_eq!(state.callouts()[0], "grant added: tool_a");
    assert!(!entries.iter().any(|e| e.contains("grant added")));

    let mut term = Terminal::new(TestBackend::new(60, 8)).unwrap();
    paint_frame(&mut term, &entries, state.callouts()).unwrap();

    let rows = rows(&term);
    let result_row = rows
        .iter()
        .position(|r| r.contains("grant added: tool_a"))
        .unwrap();
    let entry_row = rows.iter().position(|r| r.contains("entry one")).unwrap();
    assert!(
        result_row > entry_row,
        "callout must render below entry list"
    );
    assert!(!rows[entry_row].contains("grant added"));
}
