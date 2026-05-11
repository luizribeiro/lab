//! c29 — `rfl install` refuses plugin A when an existing plugin B
//! in the lock is network-open and subscribes to A's published
//! topic (scope §Tr4; security RFC §7.1.1 one-hop direct check).

mod common;

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use common::install_test_kit::{run_install, write_fixture};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantNetwork, Lock, LockFlags, PluginEntry,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::safepath::SafePath;
use rafaello_core::topic_id;

#[test]
fn rfl_install_refuses_one_hop_outbound_via_other_plugin() {
    let id_a = "local:alphapub@0.0.0";
    let pub_topic = format!("plugin.{}.update", topic_id::derive(id_a));

    let manifest_a = format!(
        r#"
schema = 1
name = "alphapub"
version = "0.0.0"
entry = "bin/x"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
read_dirs = ["${{home}}/notes"]
write_dirs = ["${{project}}/out"]

[capabilities.default.network]
mode = "deny"

[bus]
subscribes = []
publishes = ["{pub_topic}"]
"#
    );

    let id_b = CanonicalId::parse("local:bravo@0.0.0").unwrap();
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".into(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::AllowAll,
                allow_hosts: vec![],
            }),
            ..GrantBundle::default()
        },
    );
    let entry_b = PluginEntry {
        entry: SafePath::parse("bin/x").unwrap(),
        digest: format!("sha256:{}", "0".repeat(64)),
        manifest_digest: format!("sha256:{}", "1".repeat(64)),
        granted_at: Utc.with_ymd_and_hms(2026, 5, 11, 0, 0, 0).unwrap(),
        grant: Grant {
            bundles,
            subscribes: vec![pub_topic.clone()],
            publishes: vec![],
        },
        bindings: Bindings::default(),
        flags: LockFlags::default(),
    };
    let mut plugins = BTreeMap::new();
    plugins.insert(id_b, entry_b);
    let lock = Lock {
        plugins,
        session: Default::default(),
    };

    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("rafaello.lock"), lock.to_toml()).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), &manifest_a);

    let out = run_install(project.path(), fixture.path(), &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains("TrifectaRefused"),
        "stderr missing TrifectaRefused: {stderr}"
    );
}
