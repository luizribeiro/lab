//! `[provides]` raw decode (scope §M3, c05 acceptance positive).
//!
//! Tool-name and sink-class grammar checks are deferred to V1 (c10);
//! this exercises only that the typed structs decode and that the
//! `None` ≠ `Some(vec![])` distinction on `sinks` survives serde.

use rafaello_core::manifest::Manifest;

#[test]
fn provides_minimal_decodes() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep", "format"]

[provides.tool.grep]
sinks = ["workspace_write"]
grant_match = "schemas/grep.json"
always_confirm = true

[provides.tool.format]
sinks = []
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let p = m.provides.expect("provides present");
    assert_eq!(p.tools, vec!["grep".to_string(), "format".to_string()]);
    assert!(p.provider.is_none());

    let grep = p.tool.get("grep").expect("grep table");
    assert_eq!(
        grep.sinks.as_deref(),
        Some(&["workspace_write".to_string()][..])
    );
    assert_eq!(
        grep.grant_match.as_ref().map(|s| s.as_str()),
        Some("schemas/grep.json")
    );
    assert!(grep.always_confirm);

    let format = p.tool.get("format").expect("format table");
    // `Some(vec![])` ≠ `None` (pi review-2 finding 2).
    assert_eq!(format.sinks.as_deref(), Some(&[][..]));
    assert!(format.grant_match.is_none());
    assert!(!format.always_confirm);
}

#[test]
fn provides_provider_only_decodes() {
    let src = r#"
schema = 1
name = "anthropic"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
provider = "anthropic"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let p = m.provides.expect("provides present");
    assert!(p.tools.is_empty());
    assert_eq!(p.provider.as_deref(), Some("anthropic"));
    assert!(p.tool.is_empty());
}

#[test]
fn provides_absent_is_ok() {
    let src = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(m.provides.is_none());
}

#[test]
fn provides_sinks_absent_distinct_from_empty() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.grep]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let grep = m.provides.unwrap().tool.remove("grep").unwrap();
    assert!(grep.sinks.is_none());
}
