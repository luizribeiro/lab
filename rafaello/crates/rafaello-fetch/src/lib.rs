//! `rafaello-fetch` — bundled `web-fetch` tool plugin (scope §TF2, §A6).
//!
//! Test fixture env vars (accepted in production binaries, exercised only
//! by tests; no `#[cfg]` gates, no cargo feature):
//!
//! - `RFL_FETCH_TEST_BODY_PATH` — file whose contents are returned as the
//!   `web-fetch` response body. Unset, missing, or unreadable → handler
//!   returns `{ok: false, error: "fetch_test_body_unavailable"}`.
//! - `RFL_FETCH_TEST_LOG_PATH` — if set, each `handle_web_fetch` call
//!   appends `web-fetch: <url>\n` to this file. Best-effort: write
//!   failures `tracing::warn!` and continue.
//! - `RFL_FETCH_TEST_TAINT_OVERRIDE` — JSON `Vec<TaintEntry>` consumed
//!   once per process to override the taint attached to the published
//!   tool_result. Malformed JSON `tracing::error!`s and falls back to
//!   None.

use std::io::Write;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};
use ulid::Ulid;

pub use rafaello_core::bus::{JsonRpcId, TaintEntry};

pub fn handle_web_fetch(args: &Value) -> Value {
    let url = args
        .get("args")
        .and_then(|a| a.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    maybe_write_invocation_log(url);
    let path = match std::env::var("RFL_FETCH_TEST_BODY_PATH") {
        Ok(p) => p,
        Err(_) => return fetch_unavailable(),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => json!({"ok": true, "content": content}),
        Err(_) => fetch_unavailable(),
    }
}

fn fetch_unavailable() -> Value {
    json!({"ok": false, "error": "fetch_test_body_unavailable"})
}

pub fn maybe_write_invocation_log(url: &str) {
    let Ok(path) = std::env::var("RFL_FETCH_TEST_LOG_PATH") else {
        return;
    };
    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| writeln!(f, "web-fetch: {url}"))
    {
        tracing::warn!("rafaello-fetch: failed to write invocation log to {path:?}: {e}");
    }
}

static TAINT_OVERRIDE_FLAG: OnceLock<Mutex<bool>> = OnceLock::new();

pub fn take_taint_override() -> Option<Vec<TaintEntry>> {
    let raw = std::env::var("RFL_FETCH_TEST_TAINT_OVERRIDE").ok()?;
    let lock = TAINT_OVERRIDE_FLAG.get_or_init(|| Mutex::new(false));
    let mut consumed = lock.lock().ok()?;
    if *consumed {
        return None;
    }
    match serde_json::from_str::<Vec<TaintEntry>>(&raw) {
        Ok(v) => {
            *consumed = true;
            Some(v)
        }
        Err(e) => {
            tracing::error!("rafaello-fetch: malformed RFL_FETCH_TEST_TAINT_OVERRIDE JSON: {e}");
            None
        }
    }
}

#[doc(hidden)]
pub fn reset_taint_override_for_test() {
    if let Some(lock) = TAINT_OVERRIDE_FLAG.get() {
        if let Ok(mut g) = lock.lock() {
            *g = false;
        }
    }
}

pub fn compute_publish_params(
    payload: &Value,
    bus_request_id: JsonRpcId,
    result_topic: &str,
) -> Value {
    let tool_result = handle_web_fetch(payload);
    let request_id = JsonRpcId::String(Ulid::new().to_string());
    let mut params = json!({
        "topic": result_topic,
        "payload": tool_result,
        "request_id": request_id,
        "in_reply_to": [bus_request_id],
    });
    if let Some(taint) = take_taint_override() {
        params["taint"] = serde_json::to_value(taint).unwrap_or(Value::Null);
    }
    params
}
