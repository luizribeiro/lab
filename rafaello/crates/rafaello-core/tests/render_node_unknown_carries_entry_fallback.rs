//! `RenderNode::Unknown` is the server-side downgrade target for variants a
//! frontend can't paint (RFC §6). It must carry an `EntryFallback` so the
//! frontend can render `fallback.text` without inventing one.

use rafaello_core::entry::EntryFallback;
use rafaello_core::RenderNode;
use serde_json::json;

#[test]
fn unknown_serializes_with_fallback_fields() {
    let node = RenderNode::Unknown {
        kind: "custom-plot".to_string(),
        payload: json!({ "series": [1, 2, 3] }),
        fallback: EntryFallback {
            text: "Plot of three points.".to_string(),
            markdown: Some("**Plot** of three points.".to_string()),
            summary: Some("plot".to_string()),
        },
    };

    let v = serde_json::to_value(&node).unwrap();
    assert_eq!(v["node"], json!("Unknown"));
    assert_eq!(v["kind"], json!("custom-plot"));
    assert_eq!(v["fallback"]["text"], json!("Plot of three points."));
    assert_eq!(
        v["fallback"]["markdown"],
        json!("**Plot** of three points.")
    );
    assert_eq!(v["fallback"]["summary"], json!("plot"));
}

#[test]
fn unknown_round_trips_with_fallback_optional_fields_omitted() {
    let v = json!({
        "node": "Unknown",
        "kind": "diagram",
        "payload": { "lang": "mermaid", "src": "graph TD;A-->B" },
        "fallback": { "text": "diagram: A -> B" }
    });
    let node: RenderNode = serde_json::from_value(v).unwrap();
    match node {
        RenderNode::Unknown { kind, fallback, .. } => {
            assert_eq!(kind, "diagram");
            assert_eq!(fallback.text, "diagram: A -> B");
            assert!(fallback.markdown.is_none());
            assert!(fallback.summary.is_none());
        }
        _ => panic!("expected Unknown"),
    }
}
