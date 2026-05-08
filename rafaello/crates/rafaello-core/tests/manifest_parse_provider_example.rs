//! Worked example: a provider plugin manifest (scope §M3,
//! c11 positive).

use rafaello_core::manifest::Manifest;

const SRC: &str = r#"
schema = 1
name = "anthropic"
version = "0.1.0"
entry = "bin/provider.sh"
rafaello = ">=0.1, <0.2"

[provides]
provider = "anthropic"

[bus]
publishes = ["provider.anthropic.response"]
subscribes = ["provider.anthropic.request"]
"#;

#[test]
fn provider_example_decodes() {
    let m = Manifest::parse(SRC).expect("parse");
    let provides = m.provides.as_ref().expect("provides");
    assert!(provides.tools.is_empty());
    assert_eq!(provides.provider.as_deref(), Some("anthropic"));
}
