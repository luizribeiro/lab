//! Built-in `heading` renderer (Stream E §3.1 + §4.1) — projects to
//! `RenderNode::Heading { level, text }`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn heading_renderer_projects_heading_payload() {
    let entry = Entry::new_heading(2, "section");

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Heading",
        "level": 2,
        "text": "section",
    });
    assert_eq!(actual, expected);
}
