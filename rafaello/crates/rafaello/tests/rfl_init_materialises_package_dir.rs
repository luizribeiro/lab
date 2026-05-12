//! c02 §A4 round-3 B-1 + round-4 B-1 — `rfl init` copies the bundled
//! `rfl-openai` package tree into `.rafaello/plugins/<topic-id>/` with
//! `bin/rfl-openai` as a real file (PP1 containment invariant) and
//! lock digests recomputable from the on-disk plugin tree.

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::compile;
use rafaello_core::digest;
use rafaello_core::lock::Lock;
use rafaello_core::manifest::Manifest;
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[test]
fn rfl_init_materialises_package_dir() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let manifest_src = make_bundled_openai_with_symlink(bundled.path());

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--yes")
        .arg("--project-root")
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .env("RFL_BUNDLED_BIN_OPENAI", workspace_bin("rfl-openai-stub"))
        .output()
        .expect("spawn rfl init");
    assert!(
        out.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let plugin_dir = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    let manifest_path = plugin_dir.join("rafaello.toml");
    assert!(
        manifest_path.is_file(),
        "manifest missing at {manifest_path:?}"
    );
    let manifest_raw = fs::read_to_string(&manifest_path).unwrap();
    let manifest = Manifest::parse(&manifest_raw).expect("manifest must parse");

    let entry_path = plugin_dir.join("bin").join("rfl-openai");
    let lmeta = fs::symlink_metadata(&entry_path).unwrap();
    assert!(
        lmeta.file_type().is_file() && !lmeta.file_type().is_symlink(),
        "bin/rfl-openai must be a real file, not a symlink: {lmeta:?}"
    );

    let resolved = compile::resolve_entry(&plugin_dir, "bin/rfl-openai")
        .expect("resolve_entry must accept the materialised entry");
    assert!(
        resolved.starts_with(plugin_dir.canonicalize().unwrap()),
        "resolved entry escaped plugin_dir: {resolved:?}"
    );

    let lock_raw = fs::read_to_string(project.path().join("rafaello.lock")).unwrap();
    let lock = Lock::from_toml(&lock_raw).unwrap();
    let entry = lock
        .plugins
        .iter()
        .find(|(k, _)| k.to_string() == OPENAI_CANONICAL)
        .expect("openai plugin entry present")
        .1;

    let expected_content = digest::content_digest(&plugin_dir).unwrap();
    assert_eq!(entry.digest, expected_content);
    let expected_manifest = digest::manifest_digest(&manifest.canonical_bytes());
    assert_eq!(entry.manifest_digest, expected_manifest);

    drop(manifest_src);
}

fn make_bundled_openai_with_symlink(root: &std::path::Path) -> std::path::PathBuf {
    let plugin_dir = root.join("openai");
    fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    fs::create_dir_all(plugin_dir.join("schemas")).unwrap();
    fs::write(
        plugin_dir.join("rafaello.toml"),
        r#"schema = 1
name = "openai"
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
    let real_bin = plugin_dir.join("bin").join("rfl-openai");
    fs::write(&real_bin, "#!/bin/sh\nexec \"$@\"\n").unwrap();
    fs::write(plugin_dir.join("openrpc.json"), "{}").unwrap();
    fs::write(plugin_dir.join("schemas").join("tool.json"), "{}").unwrap();
    plugin_dir
}
