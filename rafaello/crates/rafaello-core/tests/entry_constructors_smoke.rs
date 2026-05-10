//! Smoke-test each `Entry::new_*` constructor: assert author per kind,
//! `stream_state = Final`, fresh ULID id, and a recent `created_at`.

use chrono::Utc;
use rafaello_core::entry::EntryFallback;
use rafaello_core::{Entry, EntryAuthor, RenderNode, StreamState, ToolCallStatus};
use serde_json::json;

fn assert_common(entry: &Entry, expected_author: EntryAuthor) {
    assert_eq!(entry.metadata.author, expected_author);
    assert_eq!(entry.metadata.stream_state, StreamState::Final);
    assert!(entry.metadata.tags.is_empty());
    assert!(entry.parent.is_none());

    // ULID timestamp is ms since epoch; `created_at` should be within a
    // few seconds of now for a freshly built entry.
    let now = Utc::now();
    let drift = (now - entry.metadata.created_at).num_seconds().abs();
    assert!(drift < 5, "created_at drift too large: {drift}s");

    // Fresh ULID — non-zero and parses round-trip via Display.
    assert_ne!(entry.id.0, 0);
    let s = entry.id.to_string();
    let parsed: ulid::Ulid = s.parse().unwrap();
    assert_eq!(parsed, entry.id);
}

#[test]
fn new_text() {
    let e = Entry::new_text("hello");
    assert_eq!(e.kind, "text");
    assert_eq!(e.payload["text"], json!("hello"));
    assert_common(&e, EntryAuthor::Assistant);
}

#[test]
fn new_heading() {
    let e = Entry::new_heading(2, "Section");
    assert_eq!(e.kind, "heading");
    assert_eq!(e.payload["level"], json!(2));
    assert_eq!(e.payload["text"], json!("Section"));
    assert_common(&e, EntryAuthor::Assistant);
}

#[test]
fn new_code_block() {
    let e = Entry::new_code_block("fn main() {}", Some("rust"));
    assert_eq!(e.kind, "code_block");
    assert_eq!(e.payload["lang"], json!("rust"));
    assert_common(&e, EntryAuthor::Assistant);

    let e2 = Entry::new_code_block("plain", None);
    assert!(e2.payload.get("lang").is_none() || e2.payload["lang"].is_null());
}

#[test]
fn new_thinking() {
    let e = Entry::new_thinking("...");
    assert_eq!(e.kind, "thinking");
    assert_common(&e, EntryAuthor::Assistant);
}

#[test]
fn new_tool_call() {
    let e = Entry::new_tool_call(
        "call-7",
        "fs.read",
        json!({ "path": "/tmp/x" }),
        ToolCallStatus::Pending,
    );
    assert_eq!(e.kind, "tool_call");
    assert_eq!(e.payload["id"], json!("call-7"));
    assert_eq!(e.payload["status"], json!("pending"));
    assert_common(&e, EntryAuthor::Assistant);
}

#[test]
fn new_tool_result() {
    let e = Entry::new_tool_result(
        "call-7",
        true,
        RenderNode::Text {
            text: "ok".to_string(),
            emphasis: None,
        },
    );
    assert_eq!(e.kind, "tool_result");
    assert_eq!(e.payload["call_id"], json!("call-7"));
    assert_eq!(e.payload["ok"], json!(true));
    assert_common(&e, EntryAuthor::Tool);
}

#[test]
fn new_image() {
    let e = Entry::new_image("https://x/y.png", "image/png", "alt");
    assert_eq!(e.kind, "image");
    assert_eq!(e.payload["uri"], json!("https://x/y.png"));
    assert_common(&e, EntryAuthor::Assistant);
}

#[test]
fn new_error() {
    let e = Entry::new_error("E_BAD", "boom");
    assert_eq!(e.kind, "error");
    assert_eq!(e.payload["code"], json!("E_BAD"));
    assert_eq!(e.payload["message"], json!("boom"));
    assert_common(&e, EntryAuthor::System);
}

#[test]
fn new_unknown() {
    let e = Entry::new_unknown(
        "custom",
        json!({ "x": 1 }),
        EntryFallback {
            text: "fallback".to_string(),
            markdown: None,
            summary: None,
        },
    );
    assert_eq!(e.kind, "custom");
    assert_eq!(e.payload, json!({ "x": 1 }));
    assert_eq!(
        e.fallback.as_ref().expect("fallback is set").text,
        "fallback"
    );
    assert_common(&e, EntryAuthor::Plugin);
}

#[test]
fn ids_are_unique_per_constructor_call() {
    let a = Entry::new_text("x");
    let b = Entry::new_text("x");
    assert_ne!(a.id, b.id);
}
