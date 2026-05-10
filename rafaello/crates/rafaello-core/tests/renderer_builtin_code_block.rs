//! Built-in `code_block` renderer (Stream E §3.1 + §4.1) — projects to
//! `RenderNode::Code { code, lang }`.

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn code_block_renderer_projects_with_lang() {
    let entry = Entry::new_code_block("fn main() {}", Some("rust"));

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Code",
        "code": "fn main() {}",
        "lang": "rust",
    });
    assert_eq!(actual, expected);
}

#[test]
fn code_block_renderer_omits_lang_when_absent() {
    let entry = Entry::new_code_block("plain text", None);

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let actual = serde_json::to_value(&out).unwrap();
    let expected = json!({
        "node": "Code",
        "code": "plain text",
    });
    assert_eq!(actual, expected);
}
