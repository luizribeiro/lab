//! Path B — registered renderer panics. The pipeline must catch it via
//! `std::panic::catch_unwind`, log at `tracing::error!`, and fall through to
//! Path A (default callout, since this entry has no fallback).

use std::sync::Arc;

use rafaello_core::entry::{Entry, RenderNode};
use rafaello_core::{Capabilities, RenderPipeline, Renderer, RendererError, RendererRegistry};

struct PanickingRenderer;

impl Renderer for PanickingRenderer {
    fn render(&self, _entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        panic!("renderer boom");
    }
}

#[tracing_test::traced_test]
#[test]
fn panic_logs_error_and_falls_into_path_a() {
    let mut registry = RendererRegistry::new();
    registry.register("test:panic".into(), Arc::new(PanickingRenderer));
    let pipeline = RenderPipeline::new(Arc::new(registry));

    let mut entry = Entry::new_text("ignored");
    entry.kind = "test:panic".into();
    entry.fallback = None;

    let out = pipeline.render(&entry, &Capabilities::tui_default());
    assert!(matches!(out, RenderNode::Callout { .. }));

    assert!(logs_contain("renderer panicked"));
    assert!(logs_contain("test:panic"));
}
