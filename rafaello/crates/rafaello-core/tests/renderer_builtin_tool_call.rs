//! Built-in `tool_call` renderer (Stream E §3.1 + §4.1) — projects to a
//! `KeyValue` node with `name`, pretty-printed `args`, and `status`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::entry::ToolCallStatus;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn tool_call_renderer_projects_keyvalue() {
    let entry = Entry::new_tool_call(
        "call-1",
        "fs.read",
        json!({ "path": "/etc/hosts" }),
        ToolCallStatus::Running,
    );

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let args_pretty = serde_json::to_string_pretty(&json!({ "path": "/etc/hosts" })).unwrap();
    let expected = json!({
        "node": "KeyValue",
        "pairs": [
            { "key": "name", "value": { "node": "Text", "text": "fs.read" } },
            { "key": "args", "value": { "node": "Text", "text": args_pretty } },
            { "key": "status", "value": { "node": "Text", "text": "running" } },
        ],
    });
    assert_eq!(actual, expected);
}
