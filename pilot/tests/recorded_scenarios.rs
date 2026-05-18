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
//!
//! Note on the `cassette!()` macro: it derives the fixture path from the
//! enclosing function's name via `type_name`. Always call `cassette!(...)`
//! at the per-test call site, not inside a shared helper — otherwise all
//! tests collapse onto the same fixture path.

use futures_util::StreamExt;
use pilot::{Claude, Codex, Driver, Event, Gemini, Pi, Session, TurnItem, TurnOptions, cassette};

#[tokio::test]
async fn claude_invalid_model_yields_failed_turn_complete() {
    invalid_model(cassette!(Claude::new())).await;
}

#[tokio::test]
async fn codex_invalid_model_yields_failed_turn_complete() {
    invalid_model(cassette!(Codex::new())).await;
}

#[tokio::test]
async fn gemini_invalid_model_yields_failed_turn_complete() {
    invalid_model(cassette!(Gemini::new())).await;
}

// pi exits silently with no stream-json on invalid --model (documented
// silent-error limitation, see Pi driver rustdoc). No events to pin,
// so there's nothing useful to record.
#[ignore = "pi emits no events on invalid model (silent-error limitation)"]
#[tokio::test]
async fn pi_invalid_model_yields_failed_turn_complete() {
    invalid_model(cassette!(Pi::new())).await;
}

#[tokio::test]
async fn claude_happy_path_says_hi() {
    happy_path(cassette!(Claude::new())).await;
}

#[tokio::test]
async fn codex_happy_path_says_hi() {
    happy_path(cassette!(Codex::new())).await;
}

#[tokio::test]
async fn gemini_happy_path_says_hi() {
    happy_path(cassette!(Gemini::new())).await;
}

#[tokio::test]
async fn pi_happy_path_says_hi() {
    happy_path(cassette!(Pi::new())).await;
}

async fn invalid_model<D: Driver + 'static>(driver: D) {
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

async fn happy_path<D: Driver + 'static>(driver: D) {
    let mut session = Session::new(driver, "/tmp");

    let mut opts = TurnOptions::default();
    opts.timeout = Some(std::time::Duration::from_secs(60));

    let mut stream = session
        .send("Say only the word: hi", opts)
        .await
        .expect("send failed");

    let mut saw_text = false;
    let mut saw_ok_complete = false;
    let mut events: Vec<Event> = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(TurnItem::Event(e)) => {
                if matches!(e, Event::AssistantText { .. }) {
                    saw_text = true;
                }
                if matches!(e, Event::TurnComplete { ok: true }) {
                    saw_ok_complete = true;
                }
                events.push(e);
            }
            Ok(TurnItem::Complete(_)) => {}
            Ok(_) => {}
            Err(_) => {}
        }
    }

    assert!(
        saw_text,
        "no AssistantText event observed. events: {events:?}"
    );
    assert!(
        saw_ok_complete,
        "no TurnComplete{{ok:true}} observed. events: {events:?}"
    );
}
