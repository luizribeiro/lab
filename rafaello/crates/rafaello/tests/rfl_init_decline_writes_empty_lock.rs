//! c03 §A4 decline arm — `rfl init` without `--yes`, with `n\n` on
//! stdin, writes an empty lock and skips the PP1 copy.

mod common;

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::Lock;

#[test]
fn rfl_init_decline_writes_empty_lock() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai(bundled.path());

    let rfl = workspace_bin("rfl");
    let mut child = Command::new(rfl)
        .arg("init")
        .arg("--project-root")
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl init");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"n\n")
        .expect("write stdin");
    let out = child.wait_with_output().expect("rfl init wait");
    assert!(
        out.status.success(),
        "rfl init decline failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock_raw = fs::read_to_string(project.path().join("rafaello.lock")).unwrap();
    let lock = Lock::from_toml(&lock_raw).expect("empty lock must parse");
    assert!(
        lock.plugins.is_empty(),
        "declined lock must have no plugin entries"
    );
    assert!(
        lock.session.provider_active.is_none(),
        "declined lock must have no provider_active"
    );

    let plugins_root = project.path().join(".rafaello").join("plugins");
    if plugins_root.exists() {
        let entries: Vec<_> = fs::read_dir(&plugins_root).unwrap().collect();
        assert!(
            entries.is_empty(),
            ".rafaello/plugins/ must be empty on decline; got {entries:?}"
        );
    }
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
