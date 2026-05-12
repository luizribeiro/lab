//! c04 §A4 closing assertion — `rfl init --yes` against a
//! synthetic bundled-source tempdir (constructed in-test, no
//! dependency on the in-tree `crates/rafaello-openai/` files
//! which land in c06) writes a lock whose `manifest_digest`
//! field matches `digest::manifest_digest` over the canonical
//! bytes of the materialised plugin manifest under PP1.

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::digest;
use rafaello_core::lock::Lock;
use rafaello_core::manifest::Manifest;
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[test]
fn rfl_init_writes_lock_against_synthetic_bundled_tree() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    write_synthetic_bundled_openai(bundled.path());

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--yes")
        .arg("--project-root")
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl init");
    assert!(
        out.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let materialised_manifest = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic)
        .join("rafaello.toml");
    let manifest_raw = fs::read_to_string(&materialised_manifest)
        .unwrap_or_else(|e| panic!("read {materialised_manifest:?}: {e}"));
    let manifest = Manifest::parse(&manifest_raw).expect("materialised manifest must parse");

    let lock_raw = fs::read_to_string(project.path().join("rafaello.lock")).unwrap();
    let lock = Lock::from_toml(&lock_raw).unwrap();
    let entry = lock
        .plugins
        .iter()
        .find(|(k, _)| k.to_string() == OPENAI_CANONICAL)
        .map(|(_, v)| v)
        .expect("openai plugin entry present in lock");

    let expected = digest::manifest_digest(&manifest.canonical_bytes());
    assert_eq!(
        entry.manifest_digest, expected,
        "lock manifest_digest must match digest over copied tree's canonical manifest bytes"
    );
}

fn write_synthetic_bundled_openai(root: &std::path::Path) {
    let plugin_dir = root.join("openai");
    fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    fs::write(
        plugin_dir.join("rafaello.toml"),
        r#"schema = 1
name = "rfl-openai"
version = "0.0.0"
entry = "bin/rfl-openai"
rafaello = ">=0.1, <0.2"
load = "eager"

[provides]
provider = "openai"

[bus]
subscribes = ["core.session.user_message"]
publishes = ["provider.openai.assistant_message"]

[capabilities.default.network]
mode = "proxy"
allow_hosts = ["litellm.example"]

[capabilities.default.env]
pass = []
allow_secrets = ["OPENAI_API_KEY"]
"#,
    )
    .unwrap();
    fs::write(plugin_dir.join("openrpc.json"), "{}").unwrap();
    let bin = plugin_dir.join("bin").join("rfl-openai");
    fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
