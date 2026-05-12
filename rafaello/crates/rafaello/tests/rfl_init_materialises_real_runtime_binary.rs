//! c02 §C1 — `rfl init` swaps the manifest shim for the real runtime
//! binary at materialisation time, and the lock's digest reflects the
//! post-swap install dir.

mod common;

use std::fs;

use common::workspace_bin_path::workspace_bin;
use rafaello::init::{self, InitArgs};
use rafaello_core::digest;
use rafaello_core::lock::Lock;
use rafaello_core::topic_id;
use serial_test::serial;

const ENV_PLUGINS_DIR: &str = "RFL_BUNDLED_PLUGINS_DIR";
const ENV_BIN_OPENAI: &str = "RFL_BUNDLED_BIN_OPENAI";
const CANONICAL: &str = "builtin:openai@0.0.0";

struct EnvGuard {
    keys: Vec<&'static str>,
}

impl EnvGuard {
    fn new() -> Self {
        Self { keys: Vec::new() }
    }
    fn set(&mut self, key: &'static str, value: impl AsRef<std::ffi::OsStr>) {
        std::env::set_var(key, value);
        if !self.keys.contains(&key) {
            self.keys.push(key);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for k in &self.keys {
            std::env::remove_var(k);
        }
    }
}

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
#[serial(bundled_env)]
fn rfl_init_materialises_real_runtime_binary() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai_fixture(bundled.path());
    let stub = workspace_bin("rfl-openai-stub");

    let mut g = EnvGuard::new();
    g.set(ENV_PLUGINS_DIR, bundled.path());
    g.set(ENV_BIN_OPENAI, &stub);

    init::run(InitArgs {
        yes: true,
        force: true,
        project_root: Some(project.path().to_path_buf()),
    })
    .expect("init::run must succeed");

    let topic = topic_id::derive(CANONICAL);
    let target_dir = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    let materialised = target_dir.join("bin").join("rfl-openai");

    let materialised_bytes = fs::read(&materialised).expect("read materialised entry");
    let stub_bytes = fs::read(&stub).expect("read stub bin");
    assert_eq!(
        materialised_bytes, stub_bytes,
        "materialised entry must be byte-identical to the workspace stub binary"
    );

    assert!(
        materialised_bytes.len() > 1024,
        "materialised entry is suspiciously small: {} bytes",
        materialised_bytes.len()
    );
    assert!(
        !materialised_bytes.starts_with(b"#!/bin/sh\nexec"),
        "materialised entry still looks like the manifest-validator shim"
    );

    let lock_text = fs::read_to_string(project.path().join("rafaello.lock"))
        .expect("read lock file after init");
    let parsed = Lock::from_toml(&lock_text).expect("lock must parse");
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
