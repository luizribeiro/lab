//! Top-level manifest decode (scope §M1, c04 acceptance positive).
//!
//! `[provides]` / `[bus]` / `[capabilities]` / `[load]` /
//! `[[renderers]]` are absent — only the top-level required +
//! optional fields appear, and `Manifest::parse` decodes them.

use rafaello_core::manifest::Manifest;
use semver::{Version, VersionReq};

#[test]
fn top_level_only_manifest_decodes() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"
description = "Rust tooling helpers"
authors = ["Alice <alice@example.com>", "Bob"]
license = "MIT"
homepage = "https://example.com/rust-tools"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert_eq!(m.schema, 1);
    assert_eq!(m.name, "rust-tools");
    assert_eq!(m.version, Version::new(0, 3, 1));
    assert_eq!(m.entry.as_str(), "bin/run.sh");
    assert_eq!(m.rafaello, VersionReq::parse(">=0.1, <0.2").unwrap());
    assert_eq!(m.description.as_deref(), Some("Rust tooling helpers"));
    assert_eq!(
        m.authors.as_deref(),
        Some(&["Alice <alice@example.com>".to_string(), "Bob".to_string()][..])
    );
    assert_eq!(m.license.as_deref(), Some("MIT"));
    assert_eq!(m.homepage.as_deref(), Some("https://example.com/rust-tools"));
}

#[test]
fn optional_metadata_absent_is_ok() {
    let src = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(m.description.is_none());
    assert!(m.authors.is_none());
    assert!(m.license.is_none());
    assert!(m.homepage.is_none());
}
