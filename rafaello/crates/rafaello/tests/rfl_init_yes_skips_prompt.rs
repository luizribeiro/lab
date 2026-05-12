//! c03 §A3 — `rfl init --yes` skips the install-time review prompt
//! even with stdin closed; produces the c02 happy-path lock + PP1
//! plugin tree.

mod common;

use std::fs;
use std::process::{Command, Stdio};

use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::Lock;
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[test]
fn rfl_init_yes_skips_prompt() {
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
        .stdin(Stdio::null())
        .output()
        .expect("spawn rfl init");
    assert!(
        out.status.success(),
        "rfl init --yes failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock_raw = fs::read_to_string(project.path().join("rafaello.lock")).unwrap();
    let lock = Lock::from_toml(&lock_raw).expect("lock must parse");
    assert!(
        lock.plugins
            .iter()
            .any(|(k, _)| k.to_string() == OPENAI_CANONICAL),
        "missing builtin:openai@0.0.0 plugin entry"
    );

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let plugin_dir = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    assert!(
        plugin_dir.is_dir(),
        "PP1 plugin dir missing at {plugin_dir:?}"
    );
    assert!(
        plugin_dir.join("bin").join("rfl-openai").is_file(),
        "PP1 entry binary missing"
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
