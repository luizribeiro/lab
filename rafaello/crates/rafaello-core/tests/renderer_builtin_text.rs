//! Built-in `text` renderer (Stream E §3.1 + §4.1) — produces
//! `RenderNode::Text { text, emphasis: None }` from the payload's `text`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn text_renderer_projects_text_payload() {
    let entry = Entry::new_text("hello world");

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Text",
        "text": "hello world",
    });
    assert_eq!(actual, expected);
}
