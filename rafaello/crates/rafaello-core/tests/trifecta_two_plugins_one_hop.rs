//! c19 — two-plugin programmatic lock exercising the §Tr3 one-hop
//! direct check. Plugin A has workspace_write + reads_untrusted
//! (read_dirs outside `${project}`) and `network.mode = "deny"`,
//! but publishes a topic that plugin B subscribes; B has
//! `network.mode = "proxy"`. Trifecta on A → all three bools true,
//! refuse = true.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantFilesystem, GrantNetwork, Lock, LockFlags,
    PluginEntry,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::safepath::SafePath;
use rafaello_core::paths::PathContext;
use rafaello_core::trifecta::{evaluate, TrifectaState};

fn ctx() -> PathContext {
    PathContext {
        project_root: PathBuf::from("/work/proj"),
        home: PathBuf::from("/home/u"),
        plugin_dir: PathBuf::from("/work/proj/.rafaello/plugins/a"),
        cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
        state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
    }
}

fn entry(grant: Grant, flags: LockFlags) -> PluginEntry {
    PluginEntry {
        entry: SafePath::parse("bin/x.js").unwrap(),
        digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_owned(),
        manifest_digest: "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
            .to_owned(),
        granted_at: Utc.with_ymd_and_hms(2026, 1, 15, 8, 30, 0).unwrap(),
        grant,
        bindings: Bindings::default(),
        flags,
    }
}

fn fixture() -> (Lock, CanonicalId) {
    let id_a = CanonicalId::parse("github.com/acme:writer@1.0.0").unwrap();
    let id_b = CanonicalId::parse("github.com/acme:relay@1.0.0").unwrap();

    let mut a_bundles = BTreeMap::new();
    a_bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${home}/notes".to_owned()],
                write_dirs: vec!["${project}/out".to_owned()],
                ..GrantFilesystem::default()
            }),
            network: Some(GrantNetwork {
                mode: NetworkMode::Deny,
                allow_hosts: Vec::new(),
            }),
            ..GrantBundle::default()
        },
    );
    let grant_a = Grant {
        bundles: a_bundles,
        publishes: vec!["plugin.id_writer.update".to_owned()],
        subscribes: Vec::new(),
    };

    let mut b_bundles = BTreeMap::new();
    b_bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    let grant_b = Grant {
        bundles: b_bundles,
        publishes: Vec::new(),
        subscribes: vec!["plugin.id_writer.*".to_owned()],
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(id_a.clone(), entry(grant_a, LockFlags::default()));
    plugins.insert(id_b, entry(grant_b, LockFlags::default()));

    (
        Lock {
            plugins,
            session: Default::default(),
        },
        id_a,
    )
}

#[test]
fn one_hop_outbound_drives_refusal() {
    let (lock, id_a) = fixture();
    let state = evaluate(&lock, &id_a, &ctx());
    assert_eq!(
        state,
        TrifectaState {
            reads_untrusted: true,
            has_outbound: true,
            has_workspace_write: true,
            refuse: true,
        }
    );
}
