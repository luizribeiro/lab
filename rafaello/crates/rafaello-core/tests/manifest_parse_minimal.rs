//! Minimal manifest decode + canonical_bytes round-trip
//! (scope §M1, §M9; lifted from c04 per c11 acceptance).

use rafaello_core::manifest::Manifest;

const MINIMAL: &str = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;

#[test]
fn minimal_manifest_parses() {
    let m = Manifest::parse(MINIMAL).expect("parse should succeed");
    assert_eq!(m.schema, 1);
    assert_eq!(m.name, "minimal");
    assert_eq!(m.entry.as_str(), "main.py");
}

#[test]
fn minimal_manifest_canonical_bytes_round_trip() {
    let m = Manifest::parse(MINIMAL).expect("parse");
    let bytes = m.canonical_bytes();
    let s = std::str::from_utf8(&bytes).expect("utf-8 output");
    let reparsed = Manifest::parse(s).expect("canonical bytes re-parse");
    assert_eq!(m, reparsed);
}
