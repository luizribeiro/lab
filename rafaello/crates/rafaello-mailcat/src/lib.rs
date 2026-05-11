//! `rafaello-mailcat` — bundled `send-mail` tool plugin (scope §TP1–§TP4).
//!
//! Appends each `tool_request` payload to `mailcat.log` under the
//! per-plugin private state dir. No actual SMTP. Returns
//! `{ok: false, error: "missing 'to' field"}` if the request omits
//! `args.to`.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

pub const LOG_FILE_NAME: &str = "mailcat.log";

pub fn handle_tool_request(payload: &Value, private_state_dir: &Path) -> Value {
    let to = payload
        .get("args")
        .and_then(|a| a.get("to"))
        .and_then(|v| v.as_str());
    if to.is_none() {
        return json!({"ok": false, "error": "missing 'to' field"});
    }
    let log_path = private_state_dir.join(LOG_FILE_NAME);
    let line = serde_json::to_string(payload).unwrap_or_else(|_| "null".to_string());
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| writeln!(f, "{line}"))
    {
        Ok(()) => json!({"ok": true}),
        Err(e) => json!({"ok": false, "error": format!("io error: {e}")}),
    }
}
