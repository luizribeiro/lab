//! `entry` parent-dir traversal is rejected at parse time via
//! `SafePath` (scope §M11, c11 negative).

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::Manifest;

#[test]
fn entry_parent_dir_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "../escape.sh"
rafaello = ">=0.1, <0.2"
"#;
    let err = Manifest::parse(src).expect_err("must reject parent-dir entry");
    match err {
        ManifestError::Toml(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("parent-dir"),
                "expected parent-dir message, got: {msg}"
            );
        }
        other => panic!("expected Toml-wrapped SafePath error, got {other:?}"),
    }
}

#[test]
fn entry_leading_slash_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "/etc/passwd"
rafaello = ">=0.1, <0.2"
"#;
    let err = Manifest::parse(src).expect_err("must reject absolute entry");
    match err {
        ManifestError::Toml(e) => {
            assert!(e.to_string().contains("leading slash"));
        }
        other => panic!("expected Toml-wrapped SafePath error, got {other:?}"),
    }
}
