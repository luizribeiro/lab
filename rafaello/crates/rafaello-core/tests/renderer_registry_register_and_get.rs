use std::sync::Arc;

use rafaello_core::entry::{Entry, RenderNode};
use rafaello_core::{Capabilities, Renderer, RendererError, RendererRegistry};

struct NullRenderer;

impl Renderer for NullRenderer {
    fn render(&self, _entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        Err(RendererError::Internal {
            detail: "test".into(),
        })
    }
}

#[test]
fn renderer_registry_register_and_get() {
    let mut registry = RendererRegistry::new();
    let renderer: Arc<dyn Renderer> = Arc::new(NullRenderer);
    let prior = registry.register("synthetic".to_string(), Arc::clone(&renderer));
    assert!(prior.is_none());

    let fetched = registry.get("synthetic").expect("registered renderer");
    assert!(Arc::ptr_eq(fetched, &renderer));
    assert!(registry.get("missing").is_none());
}
