//! c22 §CG4a: assert the `synthesise_deny_tool_result` helper
//! returns the pinned wire shape — `request_id Some`, `in_reply_to
//! = [held.tool_request.request_id]`, `taint` non-empty with
//! `source: "system"`, payload `{ok: false, error, content: ""}`.

use std::time::{Duration, Instant};

use rafaello_core::bus::{BusEvent, JsonRpcId, PublisherIdentity, TaintEntry};
use rafaello_core::gate::{synthesise_deny_tool_result, DenyReason, HeldConfirmation};
use rafaello_core::lock::canonical_id::CanonicalId;
use ulid::Ulid;

fn held_with_id(tool_request_id: JsonRpcId) -> HeldConfirmation {
    HeldConfirmation {
        tool_request: BusEvent {
            topic: "core.session.tool_request".into(),
            payload: serde_json::json!({"tool": "send_mail", "args": {}}),
            publisher: PublisherIdentity::Core,
            in_reply_to: None,
            taint: Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            request_id: Some(tool_request_id),
        },
        deadline: Instant::now() + Duration::from_secs(60),
        dispatch_target: CanonicalId::parse("local/test:mailer@0.1.0").unwrap(),
    }
}

#[test]
fn synthesise_deny_tool_result_user_denied_shape() {
    let tool_request_id = JsonRpcId::from(Ulid::new().to_string());
    let held = held_with_id(tool_request_id.clone());

    let args = synthesise_deny_tool_result(&held, DenyReason::UserDenied);

    assert_eq!(args.topic, "core.session.tool_result");
    assert_eq!(args.payload["ok"], serde_json::json!(false));
    assert_eq!(args.payload["error"], serde_json::json!("user_denied"));
    assert_eq!(args.payload["content"], serde_json::json!(""));

    assert!(args.request_id.is_some(), "fresh result id required");
    assert_ne!(
        args.request_id.as_ref().unwrap(),
        &tool_request_id,
        "result id is fresh, not the held request id",
    );

    let in_reply_to = args.in_reply_to.expect("in_reply_to set");
    assert_eq!(in_reply_to.len(), 1);
    assert_eq!(in_reply_to[0], tool_request_id);

    let taint = args.taint.expect("taint set");
    assert!(!taint.is_empty());
    assert_eq!(taint[0].source, "system");
    assert_eq!(taint[0].detail.as_deref(), Some("user_denied"));
}

#[test]
fn synthesise_deny_tool_result_confirm_timeout_shape() {
    let tool_request_id = JsonRpcId::from(Ulid::new().to_string());
    let held = held_with_id(tool_request_id);

    let args = synthesise_deny_tool_result(&held, DenyReason::ConfirmTimeout);

    assert_eq!(args.payload["error"], serde_json::json!("confirm_timeout"));
    let taint = args.taint.expect("taint set");
    assert_eq!(taint[0].source, "system");
    assert_eq!(taint[0].detail.as_deref(), Some("confirm_timeout"));
}
