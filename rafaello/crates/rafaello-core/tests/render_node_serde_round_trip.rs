//! Round-trip every `RenderNode` variant through JSON. Locks both the
//! 15-variant set from RFC §4.1 and the `node`-tagged encoding from §4.2.

use rafaello_core::entry::render_node::{CalloutKind, Emphasis, KeyValuePair};
use rafaello_core::entry::EntryFallback;
use rafaello_core::{RawFormat, RenderNode};
use serde_json::json;

fn round_trip(node: RenderNode) -> RenderNode {
    let encoded = serde_json::to_value(&node).expect("serialize");
    serde_json::from_value(encoded).expect("deserialize")
}

#[test]
fn text_round_trip() {
    let n = RenderNode::Text {
        text: "hi".to_string(),
        emphasis: Some(Emphasis::Strong),
    };
    let value = serde_json::to_value(&n).unwrap();
    assert_eq!(value["node"], json!("Text"));
    assert!(matches!(round_trip(n), RenderNode::Text { .. }));
}

#[test]
fn heading_round_trip() {
    let n = RenderNode::Heading {
        level: 3,
        text: "h".to_string(),
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Heading { level: 3, .. }
    ));
}

#[test]
fn code_round_trip() {
    let n = RenderNode::Code {
        code: "x".to_string(),
        lang: Some("rust".to_string()),
    };
    assert!(matches!(round_trip(n), RenderNode::Code { .. }));
}

#[test]
fn inline_round_trip() {
    let n = RenderNode::Inline {
        children: vec![RenderNode::Text {
            text: "x".to_string(),
            emphasis: None,
        }],
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Inline { children } if children.len() == 1
    ));
}

#[test]
fn block_round_trip() {
    let n = RenderNode::Block {
        children: vec![RenderNode::Divider {}],
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Block { children } if children.len() == 1
    ));
}

#[test]
fn list_round_trip() {
    let n = RenderNode::List {
        ordered: true,
        items: vec![RenderNode::Text {
            text: "a".to_string(),
            emphasis: None,
        }],
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::List { ordered: true, items } if items.len() == 1
    ));
}

#[test]
fn key_value_round_trip() {
    let n = RenderNode::KeyValue {
        pairs: vec![KeyValuePair {
            key: "k".to_string(),
            value: RenderNode::Text {
                text: "v".to_string(),
                emphasis: None,
            },
        }],
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::KeyValue { pairs } if pairs.len() == 1
    ));
}

#[test]
fn table_round_trip() {
    let n = RenderNode::Table {
        headers: vec!["h".to_string()],
        rows: vec![vec![RenderNode::Text {
            text: "x".to_string(),
            emphasis: None,
        }]],
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Table { rows, .. } if rows.len() == 1
    ));
}

#[test]
fn divider_round_trip() {
    let n = RenderNode::Divider {};
    assert!(matches!(round_trip(n), RenderNode::Divider {}));
}

#[test]
fn image_round_trip() {
    let n = RenderNode::Image {
        uri: "u".to_string(),
        mime: "image/png".to_string(),
        alt: "a".to_string(),
        bytes_b64: None,
    };
    assert!(matches!(round_trip(n), RenderNode::Image { .. }));
}

#[test]
fn link_round_trip() {
    let n = RenderNode::Link {
        href: "https://x".to_string(),
        child: Box::new(RenderNode::Text {
            text: "x".to_string(),
            emphasis: None,
        }),
    };
    assert!(matches!(round_trip(n), RenderNode::Link { .. }));
}

#[test]
fn callout_round_trip() {
    let n = RenderNode::Callout {
        kind: CalloutKind::Warn,
        child: Box::new(RenderNode::Block { children: vec![] }),
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Callout {
            kind: CalloutKind::Warn,
            ..
        }
    ));
}

#[test]
fn collapsed_round_trip() {
    let n = RenderNode::Collapsed {
        summary: Box::new(RenderNode::Text {
            text: "s".to_string(),
            emphasis: None,
        }),
        detail: Box::new(RenderNode::Block { children: vec![] }),
        default_open: false,
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Collapsed {
            default_open: false,
            ..
        }
    ));
}

#[test]
fn raw_round_trip() {
    let n = RenderNode::Raw {
        format: RawFormat::Ansi,
        body: "\x1b[1mx\x1b[0m".to_string(),
    };
    assert!(matches!(
        round_trip(n),
        RenderNode::Raw {
            format: RawFormat::Ansi,
            ..
        }
    ));
}

#[test]
fn unknown_round_trip() {
    let n = RenderNode::Unknown {
        kind: "custom-plot".to_string(),
        payload: json!({ "x": [1, 2] }),
        fallback: EntryFallback {
            text: "plot".to_string(),
            markdown: None,
            summary: None,
        },
    };
    assert!(matches!(round_trip(n), RenderNode::Unknown { .. }));
}

#[test]
fn node_tag_uses_node_key() {
    let n = RenderNode::Divider {};
    let v = serde_json::to_value(&n).unwrap();
    assert_eq!(v, json!({ "node": "Divider" }));
}
