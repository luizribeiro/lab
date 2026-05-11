//! c26 / scope §TUI3 + §CG7 TUI side: when two
//! `core.session.confirm_request` events arrive back-to-back, the TUI
//! only shows the queue head; the second is queued behind it, and the
//! overlay surfaces a `+1 more pending` badge.

use rafaello_tui::confirm::{paint_confirm_overlay, ConfirmQueue};
use rafaello_tui::InputMode;
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

fn req(id: &str, summary: &str) -> serde_json::Value {
    json!({
        "request_id": id,
        "summary": summary,
        "details": {
            "tool_call_id": id,
            "tool": "fs.write",
            "args": { "path": "/etc/hosts" },
            "sinks": ["fs.write"],
            "always_confirm": false,
            "taint": [],
        },
        "ttl_seconds": 60_u64,
    })
}

#[test]
fn queue_head_shown_second_request_badged() {
    let mut q = ConfirmQueue::new();
    assert!(q.enqueue(&req("CID-1", "first request")));
    assert!(q.enqueue(&req("CID-2", "second request")));

    match q.head_overlay() {
        InputMode::ConfirmOverlay {
            summary,
            queued_count,
            ..
        } => {
            assert_eq!(summary, "first request");
            assert_eq!(queued_count, 1);
        }
        other => panic!("expected ConfirmOverlay head, got {other:?}"),
    }

    let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
    paint_confirm_overlay(&mut term, &q).unwrap();
    let painted = rows(&term);
    let summary_row = painted
        .iter()
        .position(|r| r.contains("first request"))
        .expect("head summary painted");
    assert!(
        !painted.iter().any(|r| r.contains("second request")),
        "tail entries must not render"
    );
    assert!(
        painted.iter().any(|r| r.contains("+1 more pending")),
        "expected `+1 more pending` badge, rows: {painted:?}"
    );

    q.pop_head();
    let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
    paint_confirm_overlay(&mut term, &q).unwrap();
    let painted2 = rows(&term);
    assert!(painted2.iter().any(|r| r.contains("second request")));
    assert!(
        !painted2.iter().any(|r| r.contains("+1 more pending")),
        "badge should disappear once queue drains"
    );
    let _ = summary_row;
}
