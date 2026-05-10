//! Built-in `image` renderer (Stream E §3.1 + §4.1) — projects to a
//! `RenderNode::Image` with `bytes_b64: None`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn image_renderer_projects_image_node() {
    let entry = Entry::new_image("https://example.com/cat.png", "image/png", "a cat");

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Image",
        "uri": "https://example.com/cat.png",
        "mime": "image/png",
        "alt": "a cat",
    });
    assert_eq!(actual, expected);
}
