//! c26 / scope §TUI2: the overlay's TTL countdown is ticked from a
//! `tokio::time::interval(1s)`. The countdown is purely UI; deadline
//! enforcement is server-side (§CG5).

use std::sync::Arc;
use std::time::Duration;

use rafaello_tui::confirm::{paint_confirm_overlay, run_ttl_ticker, ConfirmQueue};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::json;
use tokio::sync::Mutex;

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

async fn paint(queue: &Arc<Mutex<ConfirmQueue>>) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
    paint_confirm_overlay(&mut term, &*queue.lock().await).unwrap();
    rows(&term)
}

#[tokio::test(start_paused = true)]
async fn ttl_countdown_renders_seconds() {
    let queue = Arc::new(Mutex::new(ConfirmQueue::new()));
    queue.lock().await.enqueue(&json!({
        "request_id": "C1",
        "summary": "fs.write",
        "details": {
            "tool_call_id": "C1",
            "tool": "fs.write",
            "args": {},
            "sinks": ["fs.write"],
            "always_confirm": false,
            "taint": [],
        },
        "ttl_seconds": 60_u64,
    }));

    let ticker_handle = {
        let q = queue.clone();
        tokio::spawn(async move { run_ttl_ticker(q).await })
    };
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }

    let initial = paint(&queue).await;
    assert!(
        initial.iter().any(|r| r.contains("60s")),
        "expected `60s` initially, rows: {initial:?}"
    );

    for _ in 0..10 {
        tokio::time::advance(Duration::from_millis(1_001)).await;
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }

    let after = paint(&queue).await;
    assert!(
        after.iter().any(|r| r.contains("50s")),
        "expected `50s` after 10 s, rows: {after:?}"
    );
    assert!(
        !after.iter().any(|r| r.contains("60s")),
        "old `60s` should have decremented"
    );

    ticker_handle.abort();
}
