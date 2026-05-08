//! `Manifest::canonical_bytes()` is stable under input-order
//! permutations (scope §M9).

use rafaello_core::manifest::Manifest;

const ORDER_A: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"
homepage = "https://example.com/rust-tools"
license = "MIT"
authors = ["Alice", "Bob"]
description = "Rust tooling helpers"

[provides]
tools = ["grep", "format"]

[provides.tool.grep]
always_confirm = false

[provides.tool.format]
sinks = ["workspace_write"]

[bus]
publishes = ["plugin.x.tool_response"]
subscribes = ["core.session.**"]

[capabilities.default.filesystem]
read_dirs = ["${project}"]
"#;

const ORDER_B: &str = r#"
description = "Rust tooling helpers"
authors = ["Alice", "Bob"]
license = "MIT"
homepage = "https://example.com/rust-tools"
rafaello = ">=0.1, <0.2"
entry = "bin/run.sh"
version = "0.3.1"
name = "rust-tools"
schema = 1

[bus]
subscribes = ["core.session.**"]
publishes = ["plugin.x.tool_response"]

[provides]
tools = ["grep", "format"]

[provides.tool.format]
sinks = ["workspace_write"]

[provides.tool.grep]
always_confirm = false

[capabilities.default.filesystem]
read_dirs = ["${project}"]
"#;

#[test]
fn canonical_bytes_invariant_under_key_order() {
    let a = Manifest::parse(ORDER_A).expect("parse A");
    let b = Manifest::parse(ORDER_B).expect("parse B");
    assert_eq!(a, b, "manifests differ structurally");
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "canonical bytes diverge despite equal manifests"
    );
}

#[test]
fn canonical_bytes_is_idempotent() {
    let m = Manifest::parse(ORDER_A).expect("parse");
    let once = m.canonical_bytes();
    let s = std::str::from_utf8(&once).expect("utf-8");
    let twice = Manifest::parse(s).expect("re-parse").canonical_bytes();
    assert_eq!(once, twice);
}
