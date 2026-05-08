//! c23 — Si2 drift detection. A lock entry with
//! `tool_meta.<n>.sinks_inferred = true` whose snapshotted `sinks`
//! list no longer matches `sinks::infer_defaults` over the current
//! effective grant → `ValidationError::SinkInferenceDrift`.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{
    Bindings, Grant, GrantBundle, GrantNetwork, LoadPolicy, SessionTable, ToolMeta,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn drifted_sinks_snapshot_is_rejected() {
    let id = canonical("github.com/acme:netter@1.0.0");
    let mut e = entry(&["netter"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "netter".to_owned(),
        ToolMeta {
            sinks: vec!["workspace_write".to_owned()],
            sinks_inferred: true,
            grant_match: None,
            always_confirm: false,
        },
    );
    e.bindings = Bindings {
        provider: false,
        provider_id: None,
        tools: vec!["netter".to_owned()],
        renderer_kinds: Vec::new(),
        tool_meta,
        load: LoadPolicy::default(),
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);

    let err = validate::lock(&lock, &ctx).unwrap_err();
    match err {
        ValidationError::SinkInferenceDrift {
            tool,
            expected,
            found,
        } => {
            assert_eq!(tool, "netter");
            assert_eq!(expected, vec!["network".to_owned()]);
            assert_eq!(found, vec!["workspace_write".to_owned()]);
        }
        other => panic!("expected SinkInferenceDrift, got {other:?}"),
    }
}

#[test]
fn matching_sinks_snapshot_passes() {
    let id = canonical("github.com/acme:netter@1.0.0");
    let mut e = entry(&["netter"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "netter".to_owned(),
        ToolMeta {
            sinks: vec!["network".to_owned()],
            sinks_inferred: true,
            grant_match: None,
            always_confirm: false,
        },
    );
    e.bindings = Bindings {
        provider: false,
        provider_id: None,
        tools: vec!["netter".to_owned()],
        renderer_kinds: Vec::new(),
        tool_meta,
        load: LoadPolicy::default(),
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);

    validate::lock(&lock, &ctx).expect("matching snapshot passes Si2");
}

#[test]
fn sinks_inferred_false_skips_drift_check() {
    let id = canonical("github.com/acme:netter@1.0.0");
    let mut e = entry(&["netter"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: Vec::new(),
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "netter".to_owned(),
        ToolMeta {
            sinks: vec!["workspace_write".to_owned()],
            sinks_inferred: false,
            grant_match: None,
            always_confirm: false,
        },
    );
    e.bindings = Bindings {
        provider: false,
        provider_id: None,
        tools: vec!["netter".to_owned()],
        renderer_kinds: Vec::new(),
        tool_meta,
        load: LoadPolicy::default(),
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);

    validate::lock(&lock, &ctx).expect("sinks_inferred=false → no drift check");
}
