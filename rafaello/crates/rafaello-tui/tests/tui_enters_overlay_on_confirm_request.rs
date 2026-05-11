//! c25 / scope §TUI1: observing a `core.session.confirm_request` payload
//! transitions the TUI's input mode into `ConfirmOverlay`, carrying every
//! field listed in scope §CG3.

use fittings_core::message::JsonRpcId;
use rafaello_tui::{overlay_from_confirm_request, ConfirmDetails, InputMode};
use serde_json::json;

#[test]
fn enters_overlay_on_confirm_request() {
    let payload = json!({
        "request_id": "01HZ_CONFIRM_ID",
        "what": "tool_call",
        "summary": "fs.write via fs-plugin — sinks: [fs.write]",
        "details": {
            "tool_call_id": "01HZ_TC",
            "tool": "fs.write",
            "args": { "path": "/etc/hosts" },
            "sinks": ["fs.write"],
            "always_confirm": false,
            "taint": [],
        },
        "default": "deny",
        "ttl_seconds": 60_u64,
    });

    let mode = overlay_from_confirm_request(&payload, 0).expect("overlay built");
    match mode {
        InputMode::ConfirmOverlay {
            confirm_id,
            summary,
            details,
            ttl_remaining,
            queued_count,
        } => {
            assert_eq!(confirm_id, JsonRpcId::String("01HZ_CONFIRM_ID".to_string()));
            assert_eq!(summary, "fs.write via fs-plugin — sinks: [fs.write]");
            assert_eq!(ttl_remaining, 60);
            assert_eq!(queued_count, 0);
            assert_eq!(
                details,
                ConfirmDetails {
                    tool_call_id: "01HZ_TC".to_string(),
                    tool: "fs.write".to_string(),
                    args: json!({ "path": "/etc/hosts" }),
                    sinks: vec!["fs.write".to_string()],
                    always_confirm: false,
                    taint: json!([]),
                }
            );
        }
        other => panic!("expected ConfirmOverlay, got {other:?}"),
    }
}
