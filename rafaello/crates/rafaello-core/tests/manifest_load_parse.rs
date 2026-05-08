//! `[load]` raw decode (scope §M6, c08 acceptance positive).
//!
//! Cross-ref checks (`command` ∈ `provides.tools`, `event` patterns
//! against `bus.subscribes`, `kind` against renderer kinds) and the
//! `"lazy"` shorthand expansion to "all subscribed / all provided /
//! all registered" land in V1 (c10).

use rafaello_core::manifest::{Load, Manifest};

const HEADER: &str = r#"
schema = 1
name = "loader"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;

fn parse_with(extra: &str) -> Manifest {
    let src = format!("{HEADER}\n{extra}");
    Manifest::parse(&src).expect("parse should succeed")
}

#[test]
fn load_string_eager_decodes() {
    let m = parse_with(r#"load = "eager""#);
    assert_eq!(m.load, Some(Load::Eager));
}

#[test]
fn load_string_boot_decodes() {
    let m = parse_with(r#"load = "boot""#);
    assert_eq!(m.load, Some(Load::Boot));
}

#[test]
fn load_string_manual_decodes() {
    let m = parse_with(r#"load = "manual""#);
    assert_eq!(m.load, Some(Load::Manual));
}

#[test]
fn load_string_lazy_decodes_with_empty_lazy_fields() {
    let m = parse_with(r#"load = "lazy""#);
    assert_eq!(
        m.load,
        Some(Load::Lazy {
            event: vec![],
            command: vec![],
            kind: vec![],
        })
    );
}

#[test]
fn load_table_form_decodes() {
    let m = parse_with(
        r#"
[load]
event = ["chat.message.received", "tool.*"]
command = ["grep", "find"]
kind = ["mermaid:diagram"]
"#,
    );
    assert_eq!(
        m.load,
        Some(Load::Lazy {
            event: vec![
                "chat.message.received".to_string(),
                "tool.*".to_string(),
            ],
            command: vec!["grep".to_string(), "find".to_string()],
            kind: vec!["mermaid:diagram".to_string()],
        })
    );
}

#[test]
fn load_table_form_with_partial_fields_decodes() {
    let m = parse_with(
        r#"
[load]
command = ["grep"]
"#,
    );
    assert_eq!(
        m.load,
        Some(Load::Lazy {
            event: vec![],
            command: vec!["grep".to_string()],
            kind: vec![],
        })
    );
}

#[test]
fn load_absent_is_ok() {
    let m = parse_with("");
    assert!(m.load.is_none());
}

#[test]
fn load_unknown_string_rejected() {
    let src = format!("{HEADER}\nload = \"sometimes\"\n");
    assert!(Manifest::parse(&src).is_err());
}

#[test]
fn load_unknown_field_rejected() {
    let src = format!(
        "{HEADER}\n[load]\nevent = []\nbogus = []\n"
    );
    assert!(Manifest::parse(&src).is_err());
}
