//! `[bus]` raw decode (scope §M4, c06 acceptance positive).
//!
//! Topic / pattern grammar, `core.*` namespace ACL, and the
//! pattern-vs-topic discipline are deferred to V1 (c10); this
//! exercises only that the typed struct decodes.

use rafaello_core::manifest::Manifest;

#[test]
fn bus_basic_decodes() {
    let src = r#"
schema = 1
name = "events"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
subscribes = ["chat.message.received", "tool.*", "render.**"]
publishes = ["chat.message.sent", "tool.invoked"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let b = m.bus.expect("bus present");
    assert_eq!(
        b.subscribes,
        vec![
            "chat.message.received".to_string(),
            "tool.*".to_string(),
            "render.**".to_string(),
        ]
    );
    assert_eq!(
        b.publishes,
        vec!["chat.message.sent".to_string(), "tool.invoked".to_string(),]
    );
}

#[test]
fn bus_absent_is_ok() {
    let src = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(m.bus.is_none());
}

#[test]
fn bus_empty_table_decodes_with_empty_lists() {
    let src = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let b = m.bus.expect("bus present");
    assert!(b.subscribes.is_empty());
    assert!(b.publishes.is_empty());
}

#[test]
fn bus_unknown_field_rejected() {
    let src = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
subscribes = []
publishes = []
unknown = []
"#;
    assert!(Manifest::parse(src).is_err());
}
