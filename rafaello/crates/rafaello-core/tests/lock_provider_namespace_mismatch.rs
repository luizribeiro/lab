//! c24 — V3 lock-side publish ACL: `provider.<id>.*` requires
//! `bindings.provider == true` AND `bindings.provider_id == Some(id)`.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry_with_publishes, lock_with};

#[test]
fn lock_provider_namespace_mismatched_id_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let plug = entry_with_publishes(
        &[],
        true,
        Some("anthropic"),
        &["provider.openai.tool_request"],
    );
    let lock = lock_with(vec![(a.clone(), plug)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockProviderNamespaceMismatch
    ));
}

#[test]
fn lock_provider_namespace_non_provider_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let plug = entry_with_publishes(&["alpha"], false, None, &["provider.openai.tool_request"]);
    let lock = lock_with(vec![(a.clone(), plug)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockProviderNamespaceMismatch
    ));
}
