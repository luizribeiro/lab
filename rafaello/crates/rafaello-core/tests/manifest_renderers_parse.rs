//! `[[renderers]]` raw decode (scope §M7, c09 acceptance positive).
//!
//! Built-in kind reservation and Stream E §8 prefix grammar checks
//! land in V1 (c10).

use rafaello_core::manifest::{Manifest, Renderer};

const HEADER: &str = r#"
schema = 1
name = "renderpkg"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;

fn parse_with(extra: &str) -> Manifest {
    let src = format!("{HEADER}\n{extra}");
    Manifest::parse(&src).expect("parse should succeed")
}

#[test]
fn renderers_absent_decodes_as_empty_vec() {
    let m = parse_with("");
    assert!(m.renderers.is_empty());
}

#[test]
fn renderers_array_of_tables_decodes() {
    let m = parse_with(
        r#"
[[renderers]]
kind = "mermaid:diagram"
priority = 50
method = "renderer.render"

[[renderers]]
kind = "diff:code"
"#,
    );
    assert_eq!(
        m.renderers,
        vec![
            Renderer {
                kind: "mermaid:diagram".to_string(),
                priority: 50,
                method: Some("renderer.render".to_string()),
            },
            Renderer {
                kind: "diff:code".to_string(),
                priority: 100,
                method: None,
            },
        ]
    );
}

#[test]
fn renderers_default_priority_is_100() {
    let m = parse_with(
        r#"
[[renderers]]
kind = "mermaid:diagram"
"#,
    );
    assert_eq!(m.renderers.len(), 1);
    assert_eq!(m.renderers[0].priority, 100);
    assert_eq!(m.renderers[0].method, None);
}

#[test]
fn renderers_unknown_field_rejected() {
    let src = format!(
        "{HEADER}\n[[renderers]]\nkind = \"mermaid:diagram\"\nbogus = true\n"
    );
    assert!(Manifest::parse(&src).is_err());
}
