//! `validate_with_package` rejects a provider plugin without an
//! `openrpc.json` sibling — row 31 applies to every plugin, not
//! just tool plugins (scope §M10, c11 negative).

use std::fs;

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::{validate_with_package, Manifest};

const SRC: &str = r#"
schema = 1
name = "anthropic"
version = "0.1.0"
entry = "bin/provider.sh"
rafaello = ">=0.1, <0.2"

[provides]
provider = "anthropic"
"#;

#[test]
fn provider_plugin_without_openrpc_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::write(pkg.join("bin/provider.sh"), "#!/bin/sh\n").unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect_err("must reject");
    assert!(matches!(err, ManifestError::MissingOpenRpc), "got {err:?}");
}
