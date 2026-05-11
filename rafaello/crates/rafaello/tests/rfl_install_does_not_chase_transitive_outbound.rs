//! c29 — `rfl install` accepts plugin A even when an A→B→C chain
//! exists in the lock and only C is network-open, because B does
//! NOT subscribe to A's published topic. Trifecta is one-hop
//! direct (decisions.md row 11; security RFC §7.1.1); the
//! transitive case is a deliberate non-feature.

mod common;

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use common::install_test_kit::{read_audit_rows, run_install, write_fixture};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantNetwork, Lock, LockFlags, PluginEntry,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::safepath::SafePath;
use rafaello_core::topic_id;

fn peer(grant: Grant) -> PluginEntry {
    PluginEntry {
        entry: SafePath::parse("bin/x").unwrap(),
        digest: format!("sha256:{}", "0".repeat(64)),
        manifest_digest: format!("sha256:{}", "1".repeat(64)),
        granted_at: Utc.with_ymd_and_hms(2026, 5, 11, 0, 0, 0).unwrap(),
        grant,
        bindings: Bindings::default(),
        flags: LockFlags::default(),
    }
}

fn net_bundle(mode: NetworkMode) -> BTreeMap<String, GrantBundle> {
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".into(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode,
                allow_hosts: vec![],
            }),
            ..GrantBundle::default()
        },
    );
    bundles
}

#[test]
fn rfl_install_does_not_chase_transitive_outbound() {
    let id_a = "local:alphapub@0.0.0";
    let id_b_str = "local:bravo@0.0.0";
    let id_c_str = "local:charlie@0.0.0";
    let pub_a = format!("plugin.{}.update", topic_id::derive(id_a));
    let pub_b = format!("plugin.{}.relay", topic_id::derive(id_b_str));

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
publishes = ["{pub_a}"]
"#
    );

    let entry_b = peer(Grant {
        bundles: net_bundle(NetworkMode::Deny),
        subscribes: vec![pub_a.clone()],
        publishes: vec![pub_b.clone()],
    });
    let entry_c = peer(Grant {
        bundles: net_bundle(NetworkMode::AllowAll),
        subscribes: vec![pub_b.clone()],
        publishes: vec![],
    });

    let mut plugins = BTreeMap::new();
    plugins.insert(CanonicalId::parse(id_b_str).unwrap(), entry_b);
    plugins.insert(CanonicalId::parse(id_c_str).unwrap(), entry_c);
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
    assert!(out.status.success(), "expected zero exit; stderr={stderr}");

    let rows = read_audit_rows(project.path());
    let row = rows
        .iter()
        .find(|(k, _)| k == "install_accepted")
        .expect("install_accepted row");
    assert_eq!(row.1.get("canonical"), Some(&serde_json::json!(id_a)));
}
