//! c16 / scope §CD2: provider-only taint does not fire the §AL1
//! predicate; the overlay renders no `provenance:` block (the summary
//! line already names the provider).

use rafaello_tui::confirm::{paint_confirm_overlay, ConfirmQueue};
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
fn confirm_overlay_suppresses_provider_only_taint() {
    let mut queue = ConfirmQueue::new();
    assert!(queue.enqueue(&json!({
        "request_id": "C1",
        "summary": "fs.write",
        "details": {
            "tool_call_id": "C1",
            "tool": "fs.write",
            "args": {},
            "sinks": ["fs.write"],
            "always_confirm": false,
            "taint": [
                {"source": "provider", "detail": "openai"},
            ],
        },
        "ttl_seconds": 60_u64,
    })));

    let mut term = Terminal::new(TestBackend::new(80, 14)).unwrap();
    paint_confirm_overlay(&mut term, &queue).unwrap();
    let frame = rows(&term);
    let joined = frame.join("\n");

    assert!(
        !joined.contains("provenance:"),
        "provider-only taint must not render a `provenance:` block; frame:\n{joined}"
    );
}
