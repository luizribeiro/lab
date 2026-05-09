//! c18 — pi-6 finding 5 / scope §Si1, overview §15.1.
//!
//! When a manifest declares `provides.tools = [..., "<n>"]` but
//! omits the `[provides.tool.<n>]` table, the install-time snapshot
//! for that tool falls back to defaults: `sinks_inferred = true`,
//! `sinks` = whatever `infer_defaults` computes over the tool's
//! effective grant, `grant_match = None`, `always_confirm = false`.
//!
//! This is the lock-side round-trip half — built programmatically
//! per scope §"Out of scope" (m1 fixtures don't project from
//! manifests). It asserts the snapshot a future c34-style projector
//! would produce, by computing it from `sinks::infer_defaults` over
//! the same effective bundle, then round-tripping through TOML.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use rafaello_core::lock::grant::Grant;
use rafaello_core::lock::{
    Bindings, CanonicalId, GrantBundle, GrantFilesystem, GrantNetwork, Lock, LockFlags,
    PluginEntry, ToolMeta,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::safepath::SafePath;
use rafaello_core::sinks::{effective_grant, infer_defaults};

fn fixture_grant() -> Grant {
    let mut grant = Grant::default();

    grant.bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_paths: vec!["${project}/**".to_owned()],
                write_dirs: vec!["${project}/out".to_owned()],
                ..GrantFilesystem::default()
            }),
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );

    grant
}

#[test]
fn omitted_tool_table_snapshots_inferred_defaults() {
    let grant = fixture_grant();
    let effective = effective_grant(&grant, "grep");

    let inferred_sinks = infer_defaults(&effective, &None);
    assert_eq!(
        inferred_sinks,
        vec!["network".to_owned(), "workspace_write".to_owned()],
        "sanity: default bundle authorises both classes"
    );

    let tool_meta_grep = ToolMeta {
        sinks: inferred_sinks.clone(),
        sinks_inferred: true,
        grant_match: None,
        always_confirm: false,
    };

    let mut tool_meta = BTreeMap::new();
    tool_meta.insert("grep".to_owned(), tool_meta_grep);

    let bindings = Bindings {
        provider: false,
        provider_id: None,
        tools: vec!["grep".to_owned()],
        renderer_kinds: Vec::new(),
        tool_meta,
        load: Default::default(),
    };

    let entry = PluginEntry {
        entry: SafePath::parse("bin/grep.js").expect("safepath"),
        digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_owned(),
        manifest_digest: "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
            .to_owned(),
        granted_at: Utc.with_ymd_and_hms(2026, 1, 15, 8, 30, 0).unwrap(),
        grant,
        bindings,
        flags: LockFlags::default(),
    };

    let mut plugins = BTreeMap::new();
    let id = CanonicalId::parse("github.com/acme:grep@1.4.2").expect("canonical id");
    plugins.insert(id.clone(), entry);

    let lock = Lock {
        plugins,
        session: Default::default(),
    };

    let serialised = lock.to_toml();
    let parsed = Lock::from_toml(&serialised).expect("parse");
    assert_eq!(lock, parsed, "lock round-trips through TOML");

    let snapshot = parsed
        .plugins
        .get(&id)
        .expect("plugin present")
        .bindings
        .tool_meta
        .get("grep")
        .expect("tool_meta.grep present");

    assert!(
        snapshot.sinks_inferred,
        "tool table omitted → inferred = true"
    );
    assert_eq!(
        snapshot.sinks, inferred_sinks,
        "snapshotted sinks match infer_defaults over the effective bundle"
    );
    assert!(
        snapshot.grant_match.is_none(),
        "grant_match defaults to None"
    );
    assert!(!snapshot.always_confirm, "always_confirm defaults to false");

    let recomputed = infer_defaults(
        &effective_grant(&parsed.plugins.get(&id).unwrap().grant, "grep"),
        &None,
    );
    assert_eq!(
        recomputed, snapshot.sinks,
        "recomputing inference over the round-tripped grant matches the snapshot"
    );
}
