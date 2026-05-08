//! c22 — V3 happy-path: distinct plugin_dirs for two installed plugins;
//! a missing entry triggers `MissingPluginDir` (pi-6 finding 1).

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};
use rafaello_core::lock::SessionTable;

#[test]
fn passes_with_two_plugins_and_distinct_plugin_dirs() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let b = canonical("github.com/acme:beta@1.0.0");
    let lock = lock_with(
        vec![(a.clone(), entry(&["alpha"], false, None)), (b.clone(), entry(&["beta"], false, None))],
        SessionTable::default(),
    );
    let ctx = ctx_for(&[&a, &b]);
    validate::lock(&lock, &ctx).expect("validate::lock passes");
}

#[test]
fn missing_plugin_dir_for_installed_plugin_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let b = canonical("github.com/acme:beta@1.0.0");
    let lock = lock_with(
        vec![(a.clone(), entry(&["alpha"], false, None)), (b.clone(), entry(&["beta"], false, None))],
        SessionTable::default(),
    );
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::MissingPluginDir
    ));
}
