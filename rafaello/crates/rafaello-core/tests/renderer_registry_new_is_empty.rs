use rafaello_core::RendererRegistry;

#[test]
fn renderer_registry_new_is_empty() {
    let registry = RendererRegistry::new();
    assert!(registry.get("text").is_none());
}
