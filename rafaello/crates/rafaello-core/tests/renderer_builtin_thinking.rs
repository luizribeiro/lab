//! Built-in `thinking` renderer (Stream E §3.1 + §4.1) — projects to a
//! `Collapsed` node whose summary is the literal `"thinking"` text and whose
//! detail is the payload text, with `default_open: false`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn thinking_renderer_projects_collapsed() {
    let entry = Entry::new_thinking("internal monologue");

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Collapsed",
        "summary": {
            "node": "Text",
            "text": "thinking",
        },
        "detail": {
            "node": "Text",
            "text": "internal monologue",
        },
        "default_open": false,
    });
    assert_eq!(actual, expected);
}
