//! c16 / scope §CD2 + §AL1 predicate: when the canonical taint vector
//! contains at least one entry whose `source` is NOT `"provider"`, the
//! overlay renders a `provenance:` block listing the non-provider
//! entries (rendered as `source: detail`). The provider entry is
//! suppressed (the summary line already names the provider).

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
fn confirm_overlay_renders_taint_provenance_when_predicate_fires() {
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
                {"source": "tool", "detail": "local:rafaello-fetch@0.0.0"},
            ],
        },
        "ttl_seconds": 60_u64,
    })));

    let mut term = Terminal::new(TestBackend::new(80, 16)).unwrap();
    paint_confirm_overlay(&mut term, &queue).unwrap();
    let frame = rows(&term);
    let joined = frame.join("\n");

    assert!(
        frame.iter().any(|r| r.contains("provenance:")),
        "expected `provenance:` label line; frame:\n{joined}"
    );
    assert!(
        frame
            .iter()
            .any(|r| r.contains("tool: local:rafaello-fetch@0.0.0")),
        "expected non-provider entry rendered as `tool: local:rafaello-fetch@0.0.0`; frame:\n{joined}"
    );
    assert!(
        !joined.contains("openai"),
        "provider entry must be suppressed from the overlay; frame:\n{joined}"
    );
    assert!(
        !joined.contains("provider"),
        "provider entry must be suppressed from the overlay; frame:\n{joined}"
    );
}
