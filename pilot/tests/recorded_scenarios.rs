//! Recorded scenario tests captured against real agent CLIs.
//!
//! Default mode is REPLAY against checked-in fixtures (no CLI needed).
//! To re-record after a CLI behavior change:
//!
//!     PILOT_RECORD=<substring> cargo test --features test-support <test_name>
//!
//! See `pilot::test_support::recorded_test` for the mode-selection rules.

use pilot::test_support::recorded_test::run_or_replay;
use pilot::{Claude, Event, TurnOptions};

/// Sending a request with a clearly-invalid `--model` value should surface
/// as a failed `TurnComplete` (ok: false) rather than a silent success.
#[tokio::test]
async fn claude_invalid_model_yields_failed_turn_complete() {
    let workdir = std::env::temp_dir();
    let mut opts = TurnOptions::default();
    opts.model = Some("definitely-not-a-real-model-xyz".to_string());
    opts.timeout = Some(std::time::Duration::from_secs(30));

    let turn = run_or_replay(
        Claude::new,
        "say hi",
        opts,
        workdir,
        "tests/fixtures/recorded/claude/invalid_model.jsonl",
    )
    .await;

    let last = turn
        .events
        .last()
        .expect("scenario produced no events at all");
    assert!(
        matches!(last, Event::TurnComplete { ok: false }),
        "expected final event TurnComplete {{ ok: false }}, got: {last:?}\nFull turn events: {:?}",
        turn.events
    );
}
