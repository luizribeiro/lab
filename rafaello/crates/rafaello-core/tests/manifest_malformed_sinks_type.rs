//! `sinks = [42]` type-mismatch rejection (scope §M3, c05 negative half).
//!
//! Per the commits.md "manifest_malformed_sinks split" note, the
//! type-mismatch case (non-string element in `sinks`) is exercised
//! at this commit as a serde-level `ManifestError`. The uppercase
//! grammar failure (`sinks = ["Network"]`) is c10's V1 territory
//! and is not exercised here.

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::Manifest;

#[test]
fn sinks_non_string_element_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.grep]
sinks = [42]
"#;
    let err = Manifest::parse(src).expect_err("must reject non-string sinks element");
    match err {
        ManifestError::Toml(_) => {}
        other => panic!("expected ManifestError::Toml, got {other:?}"),
    }
}
