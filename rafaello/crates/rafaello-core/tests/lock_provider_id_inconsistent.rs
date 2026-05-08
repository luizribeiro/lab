//! c26 — `bindings.provider == false` with `provider_id = Some(_)`,
//! and the inverse, both yield `ValidationError::ProviderIdInconsistent`.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn provider_id_set_without_provider_flag() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let e = entry(&[], false, Some("anthropic"));
    let lock = lock_with(vec![(a.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ProviderIdInconsistent
    ));
}

#[test]
fn provider_flag_without_provider_id() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let e = entry(&[], true, None);
    let lock = lock_with(vec![(a.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ProviderIdInconsistent
    ));
}
