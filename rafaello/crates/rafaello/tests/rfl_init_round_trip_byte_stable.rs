//! c04 §A4 closing assertion — `rfl init --yes` produces a lock that
//! round-trips `Lock::from_toml → Lock::to_toml` byte-stably; second
//! pass asserts the rendered TOML structure (canonical id from the
//! `BTreeMap<CanonicalId, _>` invariant and grant-subtable ordering)
//! matches the literal in scope §A2.

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::Lock;

#[test]
fn rfl_init_round_trip_byte_stable() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    write_synthetic_openai(bundled.path());

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

    let bytes_first = fs::read(project.path().join("rafaello.lock")).unwrap();
    let parsed =
        Lock::from_toml(std::str::from_utf8(&bytes_first).unwrap()).expect("lock must parse");
    let bytes_second = parsed.to_toml().into_bytes();
    assert_eq!(
        bytes_first, bytes_second,
        "lock not byte-stable through Lock::from_toml → to_toml round-trip"
    );

    let rendered = std::str::from_utf8(&bytes_second).unwrap();
    let plugin_header = r#"[plugin."builtin:openai@0.0.0"]"#;
    let plugin_pos = rendered
        .find(plugin_header)
        .expect("plugin table header for builtin:openai@0.0.0 must be present");
    let session_pos = rendered
        .find("[session]")
        .expect("session table must be present");
    assert!(
        plugin_pos < session_pos,
        "BTreeMap invariant: [plugin.\"…\"] tables precede [session]"
    );

    let env_pos = rendered
        .find(r#"[plugin."builtin:openai@0.0.0".grant.bundles.default.env]"#)
        .expect("grant.bundles.default.env subtable present");
    let network_pos = rendered
        .find(r#"[plugin."builtin:openai@0.0.0".grant.bundles.default.network]"#)
        .expect("grant.bundles.default.network subtable present");
    assert!(
        network_pos < env_pos,
        "grant.bundles.default.* subtables emit in §A2 literal order \
         (network before env); got network={network_pos} env={env_pos}"
    );

    assert!(rendered.contains(r#"provider_active = "builtin:openai@0.0.0""#));
}

fn write_synthetic_openai(root: &std::path::Path) {
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
    fs::write(plugin_dir.join("openrpc.json"), "{}").unwrap();
    let bin = plugin_dir.join("bin").join("rfl-openai");
    fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
