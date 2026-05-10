//! Built-in `tool_result` renderer (Stream E §3.1 + §4.1) — wraps the
//! payload's `content` RenderNode in a `Block` headed by a status `Heading`.

use std::sync::Arc;

use rafaello_core::entry::render_node::RenderNode;
use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn tool_result_renderer_wraps_content_in_block_with_heading() {
    let content = RenderNode::Text {
        text: "exit 0".into(),
        emphasis: None,
    };
    let entry = Entry::new_tool_result("call-1", true, content);

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Block",
        "children": [
            { "node": "Heading", "level": 4, "text": "tool_result call-1 (ok)" },
            { "node": "Text", "text": "exit 0" },
        ],
    });
    assert_eq!(actual, expected);
}

#[test]
fn tool_result_renderer_marks_error_in_heading() {
    let content = RenderNode::Text {
        text: "boom".into(),
        emphasis: None,
    };
    let entry = Entry::new_tool_result("call-2", false, content);

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Block",
        "children": [
            { "node": "Heading", "level": 4, "text": "tool_result call-2 (error)" },
            { "node": "Text", "text": "boom" },
        ],
    });
    assert_eq!(actual, expected);
}
