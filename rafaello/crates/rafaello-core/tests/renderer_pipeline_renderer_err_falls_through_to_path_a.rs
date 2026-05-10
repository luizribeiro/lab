//! Path B — registered renderer returns `Err(_)`. Pipeline must log at
//! `tracing::warn!` and fall through to Path A.

use std::sync::Arc;

use rafaello_core::entry::{Entry, EntryFallback, RenderNode};
use rafaello_core::{Capabilities, RenderPipeline, Renderer, RendererError, RendererRegistry};

struct ErrRenderer;

impl Renderer for ErrRenderer {
    fn render(&self, _entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        Err(RendererError::Internal {
            detail: "synthetic-error".into(),
        })
    }
}

#[tracing_test::traced_test]
#[test]
fn renderer_err_logs_warn_and_falls_into_path_a_with_fallback() {
    let mut registry = RendererRegistry::new();
    registry.register("test:err".into(), Arc::new(ErrRenderer));
    let pipeline = RenderPipeline::new(Arc::new(registry));

    let mut entry = Entry::new_text("ignored");
    entry.kind = "test:err".into();
    entry.fallback = Some(EntryFallback {
        text: "post-err fallback".into(),
        markdown: None,
        summary: None,
    });

    let out = pipeline.render(&entry, &Capabilities::tui_default());
    let RenderNode::Block { children } = out else {
        panic!("expected Block (fallback present)");
    };
    match &children[0] {
        RenderNode::Text { text, .. } => assert_eq!(text, "post-err fallback"),
        other => panic!("expected Text, got {other:?}"),
    }

    assert!(logs_contain("renderer returned Err"));
    assert!(logs_contain("test:err"));
}
