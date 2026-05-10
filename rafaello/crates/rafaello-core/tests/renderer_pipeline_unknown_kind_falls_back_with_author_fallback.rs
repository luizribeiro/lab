//! Path A — entry kind not in registry, fallback present → emit a Block
//! containing the fallback.text per Stream E §6 first bullet.

use std::sync::Arc;

use rafaello_core::entry::{Entry, EntryFallback, RenderNode};
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};

#[test]
fn unknown_kind_with_fallback_emits_block_with_text() {
    let mut entry = Entry::new_text("ignored");
    entry.kind = "synthetic:no-such-renderer".into();
    entry.fallback = Some(EntryFallback {
        text: "author-supplied fallback".into(),
        markdown: None,
        summary: None,
    });

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::new()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    match out {
        RenderNode::Block { children } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                RenderNode::Text { text, emphasis } => {
                    assert_eq!(text, "author-supplied fallback");
                    assert!(emphasis.is_none());
                }
                other => panic!("expected Text child, got {other:?}"),
            }
        }
        other => panic!("expected Block, got {other:?}"),
    }
}
