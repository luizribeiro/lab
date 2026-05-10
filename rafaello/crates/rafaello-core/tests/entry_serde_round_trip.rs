//! Round-trip every built-in payload struct through JSON to lock the
//! field shapes specified in the renderer RFC §3.1.

use rafaello_core::entry::payloads::{
    CodeBlockPayload, ErrorPayload, HeadingPayload, ImagePayload, TextPayload, ThinkingPayload,
    ToolCallPayload, ToolCallStatus, ToolResultPayload,
};
use rafaello_core::RenderNode;
use serde_json::json;

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let encoded = serde_json::to_value(value).expect("serialize");
    serde_json::from_value(encoded).expect("deserialize")
}

#[test]
fn text_payload_round_trip() {
    let v = TextPayload {
        text: "hello".to_string(),
        markdown: true,
    };
    let back: TextPayload = round_trip(&v);
    assert_eq!(back.text, "hello");
    assert!(back.markdown);
}

#[test]
fn heading_payload_round_trip() {
    let v = HeadingPayload {
        text: "section".to_string(),
        level: 2,
    };
    let back: HeadingPayload = round_trip(&v);
    assert_eq!(back.text, "section");
    assert_eq!(back.level, 2);
}

#[test]
fn code_block_payload_round_trip() {
    let v = CodeBlockPayload {
        code: "fn main() {}".to_string(),
        lang: Some("rust".to_string()),
    };
    let back: CodeBlockPayload = round_trip(&v);
    assert_eq!(back.code, "fn main() {}");
    assert_eq!(back.lang.as_deref(), Some("rust"));
}

#[test]
fn tool_call_payload_round_trip() {
    let v = ToolCallPayload {
        id: "call-1".to_string(),
        name: "fs.read".to_string(),
        args: json!({ "path": "/tmp/a" }),
        status: ToolCallStatus::Running,
    };
    let back: ToolCallPayload = round_trip(&v);
    assert_eq!(back.id, "call-1");
    assert_eq!(back.name, "fs.read");
    assert_eq!(back.args, json!({ "path": "/tmp/a" }));
    assert_eq!(back.status, ToolCallStatus::Running);
}

#[test]
fn tool_result_payload_round_trip() {
    let v = ToolResultPayload {
        call_id: "call-1".to_string(),
        ok: true,
        content: RenderNode::Text {
            text: "ok".to_string(),
            emphasis: None,
        },
        details: Some(json!({ "exit": 0 })),
    };
    let back: ToolResultPayload = round_trip(&v);
    assert_eq!(back.call_id, "call-1");
    assert!(back.ok);
    assert_eq!(back.details, Some(json!({ "exit": 0 })));
    match back.content {
        RenderNode::Text { text, .. } => assert_eq!(text, "ok"),
        _ => panic!("expected Text node"),
    }
}

#[test]
fn thinking_payload_round_trip() {
    let v = ThinkingPayload {
        text: "let me think".to_string(),
    };
    let back: ThinkingPayload = round_trip(&v);
    assert_eq!(back.text, "let me think");
}

#[test]
fn image_payload_round_trip() {
    let v = ImagePayload {
        uri: "https://example.com/x.png".to_string(),
        mime: "image/png".to_string(),
        alt: "x".to_string(),
        bytes_b64: None,
    };
    let back: ImagePayload = round_trip(&v);
    assert_eq!(back.uri, "https://example.com/x.png");
    assert_eq!(back.mime, "image/png");
    assert_eq!(back.alt, "x");
    assert!(back.bytes_b64.is_none());
}

#[test]
fn error_payload_round_trip() {
    let v = ErrorPayload {
        code: "E_FOO".to_string(),
        message: "boom".to_string(),
        data: Some(json!({ "where": "here" })),
    };
    let back: ErrorPayload = round_trip(&v);
    assert_eq!(back.code, "E_FOO");
    assert_eq!(back.message, "boom");
    assert_eq!(back.data, Some(json!({ "where": "here" })));
}

#[test]
fn payload_rejects_unknown_field() {
    let bad = json!({ "text": "hi", "markdown": false, "extra": 1 });
    let err = serde_json::from_value::<TextPayload>(bad).unwrap_err();
    assert!(
        err.to_string().contains("unknown field"),
        "expected deny_unknown_fields error, got: {err}"
    );
}
