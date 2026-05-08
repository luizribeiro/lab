//! `grant_match` SafePath rejection (scope §M3 + §M11, c05 acceptance negative).
//!
//! `[provides.tool.<n>] grant_match = "../schemas/x.json"` is rejected
//! at parse time because `SafePath::parse` raises `ManifestError`.

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::Manifest;

#[test]
fn grant_match_parent_dir_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.grep]
grant_match = "../schemas/x.json"
"#;
    let err = Manifest::parse(src).expect_err("must reject parent-dir grant_match");
    // Serde wraps SafePath errors back through `toml::de::Error`'s
    // custom-message channel, so the surfaced variant is `Toml`. The
    // structural shape (parent-dir) shows up in the rendered message.
    match err {
        ManifestError::Toml(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("parent-dir"),
                "expected parent-dir traversal in error, got: {msg}"
            );
        }
        other => panic!("expected ManifestError::Toml, got {other:?}"),
    }
}

#[test]
fn grant_match_leading_slash_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.grep]
grant_match = "/etc/passwd"
"#;
    let err = Manifest::parse(src).expect_err("must reject absolute grant_match");
    match err {
        ManifestError::Toml(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("leading slash"),
                "expected leading-slash error, got: {msg}"
            );
        }
        other => panic!("expected ManifestError::Toml, got {other:?}"),
    }
}
