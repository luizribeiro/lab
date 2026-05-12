//! c16 / scope §CD2: provenance block is clipped to 5 rows; overflow
//! is surfaced as a final `... (N more)` line. The audit row (§AL1)
//! carries the full vector.

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
fn confirm_overlay_taint_clipping() {
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
                {"source": "tool", "detail": "local:t1@0.0.0"},
                {"source": "tool", "detail": "local:t2@0.0.0"},
                {"source": "tool", "detail": "local:t3@0.0.0"},
                {"source": "tool", "detail": "local:t4@0.0.0"},
                {"source": "tool", "detail": "local:t5@0.0.0"},
                {"source": "tool", "detail": "local:t6@0.0.0"},
            ],
        },
        "ttl_seconds": 60_u64,
    })));

    let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
    paint_confirm_overlay(&mut term, &queue).unwrap();
    let frame = rows(&term);
    let joined = frame.join("\n");

    assert!(
        frame.iter().any(|r| r.contains("provenance:")),
        "expected `provenance:` label line; frame:\n{joined}"
    );
    for n in 1..=5 {
        assert!(
            joined.contains(&format!("local:t{n}@0.0.0")),
            "expected first 5 entries shown (missing t{n}); frame:\n{joined}"
        );
    }
    assert!(
        !joined.contains("local:t6@0.0.0"),
        "6th entry must be clipped; frame:\n{joined}"
    );
    assert!(
        frame.iter().any(|r| r.contains("... (1 more)")),
        "expected `... (1 more)` clip line; frame:\n{joined}"
    );
}
