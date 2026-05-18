//! End-to-end smoke tests against real agent CLIs.
//!
//! These tests are `#[ignore]`'d by default. Run with:
//!
//!     PILOT_E2E=1 cargo test --features test-support -- --ignored
//!
//! Each test:
//!   * checks `PILOT_E2E` env var, returns early if not set;
//!   * checks the relevant binary is on PATH, returns early if missing;
//!   * runs a single "say hi" turn through the public `Session` API;
//!   * asserts at least one `AssistantText` event and exactly one `Complete` arrived.

use futures_util::StreamExt;
use pilot::{Claude, Codex, Event, Gemini, Pi, Session, TurnItem, TurnOptions};

/// True if the user opted into E2E by setting `PILOT_E2E=1`.
fn e2e_enabled() -> bool {
    matches!(std::env::var("PILOT_E2E").as_deref(), Ok("1"))
}

/// True if `name` is an executable on PATH.
fn binary_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if candidate.is_file() {
            return true;
        }
    }
    false
}

/// Run a single "say hi" smoke test against `agent`. Returns early (passing)
/// if PILOT_E2E is unset or the binary is missing — the goal is "if it CAN
/// run, it MUST succeed", not "it must always run".
async fn smoke(agent: &str) {
    if !e2e_enabled() {
        eprintln!("[skip] PILOT_E2E not set");
        return;
    }
    if !binary_on_path(agent) {
        eprintln!("[skip] {agent} not on PATH");
        return;
    }

    let workdir = std::env::temp_dir();
    let mut session = match agent {
        "claude" => Session::new(Claude::new(), workdir),
        "codex" => Session::new(Codex::new(), workdir),
        "gemini" => Session::new(Gemini::new(), workdir),
        "pi" => Session::new(Pi::new(), workdir),
        other => panic!("unknown agent: {other}"),
    };

    let mut opts = TurnOptions::default();
    opts.timeout = Some(std::time::Duration::from_secs(60));
    let mut stream = session
        .send("Say only the word: hi", opts)
        .await
        .unwrap_or_else(|e| panic!("send failed for {agent}: {e:?}"));

    let mut saw_text = false;
    let mut completes = 0usize;
    while let Some(item) = stream.next().await {
        match item.unwrap_or_else(|e| panic!("stream error from {agent}: {e:?}")) {
            TurnItem::Event(Event::AssistantText { .. }) => saw_text = true,
            TurnItem::Complete(_) => completes += 1,
            _ => {}
        }
    }

    assert!(saw_text, "{agent} produced no AssistantText event");
    assert_eq!(
        completes, 1,
        "{agent} produced {completes} Complete events; expected exactly 1"
    );
}

#[tokio::test]
#[ignore = "E2E: needs claude CLI + PILOT_E2E=1"]
async fn e2e_claude_smoke() {
    smoke("claude").await;
}

#[tokio::test]
#[ignore = "E2E: needs codex CLI + PILOT_E2E=1"]
async fn e2e_codex_smoke() {
    smoke("codex").await;
}

#[tokio::test]
#[ignore = "E2E: needs gemini CLI + PILOT_E2E=1"]
async fn e2e_gemini_smoke() {
    smoke("gemini").await;
}

#[tokio::test]
#[ignore = "E2E: needs pi CLI + PILOT_E2E=1"]
async fn e2e_pi_smoke() {
    smoke("pi").await;
}
