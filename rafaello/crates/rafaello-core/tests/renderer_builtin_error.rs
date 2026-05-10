//! Built-in `error` renderer (Stream E §3.1 + §4.1) — projects to a
//! `Callout { kind: Error }` wrapping a `KeyValue` with `code` and `message`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn error_renderer_projects_callout_with_keyvalue() {
    let entry = Entry::new_error("E_NOT_FOUND", "thing missing");

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Callout",
        "kind": "error",
        "child": {
            "node": "KeyValue",
            "pairs": [
                { "key": "code", "value": { "node": "Text", "text": "E_NOT_FOUND" } },
                { "key": "message", "value": { "node": "Text", "text": "thing missing" } },
            ],
        },
    });
    assert_eq!(actual, expected);
}
