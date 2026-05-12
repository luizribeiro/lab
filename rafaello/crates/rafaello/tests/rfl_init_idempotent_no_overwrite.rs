//! c02 §A4 — running `rfl init --yes` twice in succession is idempotent:
//! the lock bytes and package-dir tree are unchanged on the second run.

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::digest;
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[test]
fn rfl_init_idempotent_no_overwrite() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai(bundled.path());

    let rfl = workspace_bin("rfl");
    run_init(&rfl, project.path(), bundled.path(), false);

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let plugin_dir = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    let lock_path = project.path().join("rafaello.lock");

    let lock_first = fs::read(&lock_path).unwrap();
    let digest_first = digest::content_digest(&plugin_dir).unwrap();

    run_init(&rfl, project.path(), bundled.path(), false);

    let lock_second = fs::read(&lock_path).unwrap();
    let digest_second = digest::content_digest(&plugin_dir).unwrap();

    assert_eq!(lock_first, lock_second, "lock bytes changed on second run");
    assert_eq!(
        digest_first, digest_second,
        "plugin-dir content digest changed on second run"
    );
}

fn run_init(
    rfl: &std::path::Path,
    project: &std::path::Path,
    bundled: &std::path::Path,
    force: bool,
) {
    let mut cmd = Command::new(rfl);
    cmd.arg("init").arg("--yes");
    if force {
        cmd.arg("--force");
    }
    let out = cmd
        .arg("--project-root")
        .arg(project)
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled)
        .output()
        .expect("spawn rfl init");
    assert!(
        out.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
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
