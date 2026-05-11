//! c26 / pi-1 M-1 / scope §TUI3 + §CG7: a queued (not-yet-shown)
//! confirm whose backing held entry is short-circuited by a freshly
//! installed `always_allow_session` grant must be silently dropped
//! from the TUI's queue when the bus-visible
//! `core.session.confirm_resolved` (pi-1 M-1) arrives — without ever
//! rendering an overlay for it.

use rafaello_tui::confirm::{paint_confirm_overlay, ConfirmQueue, CONFIRM_RESOLVED_TOPIC};
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

fn req(id: &str) -> serde_json::Value {
    json!({
        "request_id": id,
        "summary": format!("summary for {id}"),
        "details": {
            "tool_call_id": id,
            "tool": "fs.write",
            "args": {},
            "sinks": ["fs.write"],
            "always_confirm": false,
            "taint": [],
        },
        "ttl_seconds": 60_u64,
    })
}

#[test]
fn queued_pending_silently_dropped_on_confirm_resolved() {
    assert_eq!(CONFIRM_RESOLVED_TOPIC, "core.session.confirm_resolved");

    let mut q = ConfirmQueue::new();
    q.enqueue(&req("HEAD"));
    q.enqueue(&req("TAIL"));
    assert_eq!(q.len(), 2);
    assert_eq!(q.queued_count(), 1);

    let resolved = json!({
        "request_id": "TAIL",
        "reason": "grant_short_circuit",
    });
    assert!(q.handle_confirm_resolved(&resolved));

    assert_eq!(q.len(), 1, "queued tail must be dropped");
    assert_eq!(q.queued_count(), 0, "no more pending behind head");
    assert_eq!(q.head().unwrap().confirm_id.as_str(), Some("HEAD"));

    let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
    paint_confirm_overlay(&mut term, &q).unwrap();
    let painted = rows(&term);
    assert!(painted.iter().any(|r| r.contains("summary for HEAD")));
    assert!(
        !painted.iter().any(|r| r.contains("summary for TAIL")),
        "short-circuited tail must never render"
    );
    assert!(
        !painted.iter().any(|r| r.contains("+1 more pending")),
        "badge must drop with the queued entry"
    );
}

#[test]
fn confirm_resolved_for_unknown_id_is_noop() {
    let mut q = ConfirmQueue::new();
    q.enqueue(&req("HEAD"));
    let before = q.len();
    let resolved = json!({ "request_id": "NOPE", "reason": "grant_short_circuit" });
    assert!(!q.handle_confirm_resolved(&resolved));
    assert_eq!(q.len(), before);
}
