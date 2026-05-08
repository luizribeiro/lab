//! Worked example: a renderer plugin manifest (scope §M7,
//! c11 positive).

use rafaello_core::manifest::Manifest;

const SRC: &str = r#"
schema = 1
name = "mermaid-renderer"
version = "0.2.0"
entry = "bin/renderer.sh"
rafaello = ">=0.1, <0.2"

[[renderers]]
kind = "mermaid:diagram"
priority = 50

[[renderers]]
kind = "mermaid:flowchart"
method = "render.flow"
"#;

#[test]
fn renderer_example_decodes() {
    let m = Manifest::parse(SRC).expect("parse");
    assert_eq!(m.renderers.len(), 2);
    assert_eq!(m.renderers[0].kind, "mermaid:diagram");
    assert_eq!(m.renderers[0].priority, 50);
    assert_eq!(m.renderers[1].kind, "mermaid:flowchart");
    assert_eq!(m.renderers[1].priority, 100);
    assert_eq!(m.renderers[1].method.as_deref(), Some("render.flow"));
}
