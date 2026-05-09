//! c23 — V3 acceptance: a multi-plugin fixture passes `validate::lock`
//! cleanly through trifecta + carve-out + Si2 wires; an isomorphic
//! variant that flips one plugin into a trifecta-refusal shape fails.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem, GrantNetwork, SessionTable};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::topic_id;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn multi_plugin_fixture_passes_v3() {
    let writer = canonical("github.com/acme:writer@1.0.0");
    let relay = canonical("github.com/acme:relay@1.0.0");

    let mut writer_entry = entry(&["writer"], false, None);
    let mut writer_bundles = BTreeMap::new();
    writer_bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${project}/src".to_owned()],
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
    let writer_topic = topic_id::derive(&writer.to_string());
    writer_entry.grant = Grant {
        bundles: writer_bundles,
        publishes: vec![format!("plugin.{writer_topic}.update")],
        subscribes: Vec::new(),
    };

    let mut relay_entry = entry(&["relay"], false, None);
    let mut relay_bundles = BTreeMap::new();
    relay_bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    relay_entry.grant = Grant {
        bundles: relay_bundles,
        publishes: Vec::new(),
        subscribes: vec!["plugin.unrelated.*".to_owned()],
    };

    let lock = lock_with(
        vec![(writer.clone(), writer_entry), (relay.clone(), relay_entry)],
        SessionTable::default(),
    );
    let ctx = ctx_for(&[&writer, &relay]);

    validate::lock(&lock, &ctx).expect("multi-plugin fixture passes V3");
}

#[test]
fn trifecta_failing_plugin_is_refused() {
    let writer = canonical("github.com/acme:writer@1.0.0");
    let relay = canonical("github.com/acme:relay@1.0.0");

    let mut writer_entry = entry(&["writer"], false, None);
    let mut writer_bundles = BTreeMap::new();
    writer_bundles.insert(
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
    let writer_topic = topic_id::derive(&writer.to_string());
    writer_entry.grant = Grant {
        bundles: writer_bundles,
        publishes: vec![format!("plugin.{writer_topic}.update")],
        subscribes: Vec::new(),
    };

    let mut relay_entry = entry(&["relay"], false, None);
    let mut relay_bundles = BTreeMap::new();
    relay_bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    relay_entry.grant = Grant {
        bundles: relay_bundles,
        publishes: Vec::new(),
        subscribes: vec![format!("plugin.{writer_topic}.*")],
    };

    let lock = lock_with(
        vec![(writer.clone(), writer_entry), (relay.clone(), relay_entry)],
        SessionTable::default(),
    );
    let ctx = ctx_for(&[&writer, &relay]);

    let err = validate::lock(&lock, &ctx).unwrap_err();
    match err {
        ValidationError::TrifectaRefused {
            reads_untrusted,
            has_outbound,
            has_workspace_write,
        } => {
            assert!(reads_untrusted);
            assert!(has_outbound);
            assert!(has_workspace_write);
        }
        other => panic!("expected TrifectaRefused, got {other:?}"),
    }
}

#[test]
fn carveout_refusal_surfaces_through_v3() {
    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${home}".to_owned()],
                ..GrantFilesystem::default()
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);

    let err = validate::lock(&lock, &ctx).unwrap_err();
    assert!(
        matches!(err, ValidationError::CarveOutRefused),
        "got {err:?}"
    );
}
