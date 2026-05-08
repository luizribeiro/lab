//! Worked example: a tool plugin manifest decodes into the
//! expected typed `Manifest` (scope §M1–§M7, c11 positive).

use rafaello_core::manifest::{Load, Manifest, NetworkMode};

const SRC: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"
description = "Rust tooling helpers"

[provides]
tools = ["grep", "format"]

[provides.tool.grep]
sinks = []
always_confirm = false

[provides.tool.format]
sinks = ["workspace_write"]
grant_match = "schemas/format-grant.json"
always_confirm = true

[bus]
publishes = ["plugin.id_abc.tool_response"]
subscribes = ["core.session.**", "plugin.id_abc.tool_request"]

[capabilities.default.filesystem]
read_dirs = ["${project}"]

[capabilities.default.network]
mode = "deny"

[capabilities.format.filesystem]
write_dirs = ["${project}/src"]

[load]
command = ["grep", "format"]
"#;

#[test]
fn tool_example_decodes() {
    let m = Manifest::parse(SRC).expect("parse");
    assert_eq!(m.name, "rust-tools");
    let provides = m.provides.as_ref().expect("provides");
    assert_eq!(provides.tools, vec!["grep", "format"]);
    let grep = provides.tool.get("grep").expect("grep tool meta");
    assert_eq!(grep.sinks.as_deref(), Some(&[][..]));
    let format = provides.tool.get("format").expect("format tool meta");
    assert_eq!(
        format.sinks.as_deref(),
        Some(&["workspace_write".to_string()][..])
    );
    assert_eq!(
        format.grant_match.as_ref().map(|p| p.as_str()),
        Some("schemas/format-grant.json")
    );
    assert!(format.always_confirm);

    let bus = m.bus.as_ref().expect("bus");
    assert_eq!(bus.publishes.len(), 1);
    assert_eq!(bus.subscribes.len(), 2);

    let caps = m.capabilities.as_ref().expect("capabilities");
    let default = caps.get("default").expect("default bundle");
    let net = default.network.as_ref().expect("network");
    assert_eq!(net.mode, NetworkMode::Deny);
    assert!(caps.contains_key("format"));

    match m.load.as_ref().expect("load") {
        Load::Lazy { command, .. } => {
            assert_eq!(command, &vec!["grep".to_string(), "format".to_string()]);
        }
        other => panic!("expected lazy load, got {other:?}"),
    }
}
