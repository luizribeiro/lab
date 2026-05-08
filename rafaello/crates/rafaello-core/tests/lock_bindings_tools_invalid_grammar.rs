//! c26 — `bindings.tools = ["Rust-Tools"]` (uppercase) rejected per
//! V3's tool-name grammar mirror.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn lock_bindings_tools_uppercase_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let e = entry(&["Rust-Tools"], false, None);
    let lock = lock_with(vec![(a.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::IllegalToolName { .. }
    ));
}
