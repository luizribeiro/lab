//! c25 — V3 lock-side `allow_hosts` requires proxy mode (pi-5 finding 3).

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{Grant, GrantBundle, GrantNetwork, SessionTable};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn lock_allow_hosts_with_deny_mode_is_rejected() {
    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Deny,
                allow_hosts: vec!["x.example".to_owned()],
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
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockAllowHostsOutsideProxy
    ));
}

#[test]
fn lock_allow_hosts_with_allow_all_mode_is_rejected() {
    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::AllowAll,
                allow_hosts: vec!["x.example".to_owned()],
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
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockAllowHostsOutsideProxy
    ));
}
