//! c02 §A4 happy path — `rfl init --yes --project-root <tmpdir>` against
//! a fixture bundled-plugin source tree writes a lock that round-trips
//! through `Lock::from_toml` byte-stably.

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::Lock;

#[test]
fn rfl_init_writes_default_lock() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai(bundled.path());

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

    let lock_path = project.path().join("rafaello.lock");
    let bytes_first = fs::read(&lock_path).unwrap();
    let parsed = Lock::from_toml(std::str::from_utf8(&bytes_first).unwrap())
        .expect("rendered lock must parse");
    let bytes_second = parsed.to_toml().into_bytes();
    assert_eq!(
        bytes_first, bytes_second,
        "lock not byte-stable through Lock::from_toml round-trip"
    );

    assert!(
        parsed
            .plugins
            .iter()
            .any(|(k, _)| k.to_string() == "builtin:openai@0.0.0"),
        "missing builtin:openai@0.0.0 plugin entry"
    );
    assert_eq!(
        parsed.session.provider_active.as_deref(),
        Some("builtin:openai@0.0.0")
    );
}

fn setup_bundled_openai(root: &std::path::Path) {
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
        plugin_dir.join("bin").join("rfl-openai"),
        "#!/bin/sh\nexec \"$@\"\n",
    )
    .unwrap();
}
