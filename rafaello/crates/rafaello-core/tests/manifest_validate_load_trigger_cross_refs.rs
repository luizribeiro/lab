//! Positive: a manifest whose `[load]` triggers all resolve against
//! `provides.tools`, `bus.subscribes` patterns, and `[[renderers]]`
//! kinds passes V1 (scope §V1, c10 acceptance positive).

use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn load_triggers_cross_refs_resolve() {
    let src = r#"
schema = 1
name = "loader"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep", "format"]

[bus]
subscribes = ["core.session.**", "tool.invoked"]

[[renderers]]
kind = "mermaid:diagram"

[load]
event = ["core.session.started", "tool.invoked"]
command = ["grep"]
kind = ["mermaid:diagram"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    validate::manifest_standalone(&m).expect("V1 should accept this manifest");
}
