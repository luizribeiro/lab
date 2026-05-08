//! c25 — V3 lock-side unknown grant bundle key (pi-6 finding 3).

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{Grant, GrantBundle, SessionTable};
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn lock_bundle_key_not_in_default_or_tools_is_rejected() {
    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert("typo".to_owned(), GrantBundle::default());
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockUnknownBundleKey
    ));
}

#[test]
fn lock_bundle_key_matching_declared_tool_is_accepted() {
    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert("alpha".to_owned(), GrantBundle::default());
    bundles.insert("default".to_owned(), GrantBundle::default());
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);
    validate::lock(&lock, &ctx).expect("known bundle keys pass");
}
