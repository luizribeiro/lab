//! c03 §C2 — subprocess regression: `rfl init` materialises the real runtime
//! binary via the §A1 dev-fallback arm when launched with no
//! `CARGO_BIN_EXE_*` / `RFL_BUNDLED_BIN_OPENAI` env. Reproduces the owner-hit
//! `cargo run --bin rfl -- init` cold-start layout that masked D1 through m6
//! ratification.

mod common;

use std::fs;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::digest;
use rafaello_core::topic_id;

const CANONICAL: &str = "builtin:openai@0.0.0";

fn setup_bundled_openai_fixture(root: &std::path::Path) {
    let plugin_dir = root.join("openai");
    fs::create_dir_all(plugin_dir.join("bin")).unwrap();
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
    fs::write(
        plugin_dir.join("openrpc.json"),
        r#"{ "openrpc": "1.2.6", "info": { "title": "openai", "version": "0.0.0" }, "methods": [] }"#,
    )
    .unwrap();
    fs::write(
        plugin_dir.join("bin").join("rfl-openai"),
        "#!/bin/sh\nexec \"$@\"\n",
    )
    .unwrap();
}

#[test]
fn rfl_init_runtime_binary_outside_cargo_env() {
    if std::env::var_os("CARGO_TARGET_DIR")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        eprintln!(
            "rfl_init_runtime_binary_outside_cargo_env: covers default target-dir layout only — set RFL_BUNDLED_BIN_OPENAI or unset CARGO_TARGET_DIR"
        );
        return;
    }

    let rfl_path = workspace_bin("rfl");
    let _ = workspace_bin("rfl-openai");

    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai_fixture(bundled.path());
    let fixture_root = bundled.path().to_path_buf();

    let project_root = tempfile::tempdir().unwrap();

    // Why: `.env_clear()` is the load-bearing assertion of c03. Without it,
    // `CARGO_BIN_EXE_*` from the parent `cargo test` leaks through to the
    // subprocess and the §A1 dev-fallback arm is never exercised — the bug
    // self-heals. Clearing reproduces the owner-hit `cargo run --bin rfl --
    // init` shell layout where those vars are absent.
    let status = std::process::Command::new(&rfl_path)
        .arg("init")
        .arg("--yes")
        .arg("--project-root")
        .arg(project_root.path())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("RFL_BUNDLED_PLUGINS_DIR", &fixture_root)
        .status()
        .expect("spawn rfl init");
    assert!(status.success(), "rfl init subprocess failed: {status:?}");

    let topic = topic_id::derive(CANONICAL);
    let target_dir = project_root
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    let materialised = target_dir.join("bin").join("rfl-openai");

    let materialised_bytes = fs::read(&materialised).expect("read materialised entry");
    assert!(
        materialised_bytes.len() > 1024,
        "materialised entry is suspiciously small: {} bytes",
        materialised_bytes.len()
    );

    let lock_text = fs::read_to_string(project_root.path().join("rafaello.lock"))
        .expect("read lock file after init");
    let parsed = rafaello_core::lock::Lock::from_toml(&lock_text).expect("lock must parse");
    let entry = parsed
        .plugins
        .iter()
        .find(|(k, _)| k.to_string() == CANONICAL)
        .map(|(_, v)| v)
        .expect("lock missing builtin:openai@0.0.0");
    let recomputed = digest::content_digest(&target_dir).expect("recompute content_digest");
    assert_eq!(
        entry.digest, recomputed,
        "lock digest must reflect post-swap install dir bytes"
    );
}
