//! Recorded scenario tests captured against real agent CLIs.
//!
//! Default mode is REPLAY against checked-in fixtures (no CLI needed).
//! On a fresh checkout, missing fixtures are auto-recorded against the
//! real CLI on first run. CI should set `PILOT_NO_RECORD=1` to fail loudly
//! on missing fixtures instead.
//!
//! Force re-record by deleting the fixture or `PILOT_RECORD=1 cargo test`.
//!
//! Fixture path is auto-derived from the test function name:
//!     tests/fixtures/recorded/<fn_name>.jsonl

use futures_util::StreamExt;
use pilot::{Claude, Event, Session, TurnItem, TurnOptions, cassette};

#[tokio::test]
async fn claude_invalid_model_yields_failed_turn_complete() {
    let driver = cassette!(Claude::new());
    let mut session = Session::new(driver, "/tmp");

    let mut opts = TurnOptions::default();
    opts.model = Some("definitely-not-a-real-model-xyz".to_string());
    opts.timeout = Some(std::time::Duration::from_secs(30));

    let mut stream = session.send("say hi", opts).await.expect("send failed");

    let mut events: Vec<Event> = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(TurnItem::Event(e)) => events.push(e),
            Ok(TurnItem::Complete(_)) => {}
            Ok(_) => {}
            Err(_) => {}
        }
    }

    let last = events.last().expect("no events captured");
    assert!(
        matches!(last, Event::TurnComplete { ok: false }),
        "expected TurnComplete{{ok:false}}, got {last:?}\nfull events: {events:?}"
    );
}
