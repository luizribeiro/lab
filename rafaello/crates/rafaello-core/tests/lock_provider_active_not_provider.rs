//! c22 — `[session].provider_active` references an installed plugin
//! whose `bindings.provider == false` → `ProviderActiveNotProvider`.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn provider_active_non_provider_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let session = SessionTable {
        provider_active: Some("github.com/acme:alpha@1.0.0".into()),
        tool_owner: Default::default(),
    };
    let lock = lock_with(vec![(a.clone(), entry(&["alpha"], false, None))], session);
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ProviderActiveNotProvider
    ));
}

#[test]
fn provider_active_provider_without_provider_id_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let session = SessionTable {
        provider_active: Some("github.com/acme:alpha@1.0.0".into()),
        tool_owner: Default::default(),
    };
    let lock = lock_with(vec![(a.clone(), entry(&[], true, None))], session);
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ProviderActiveNotProvider
    ));
}

#[test]
fn provider_active_provider_with_provider_id_passes() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let session = SessionTable {
        provider_active: Some("github.com/acme:alpha@1.0.0".into()),
        tool_owner: Default::default(),
    };
    let lock = lock_with(vec![(a.clone(), entry(&[], true, Some("anthropic")))], session);
    let ctx = ctx_for(&[&a]);
    validate::lock(&lock, &ctx).expect("provider_active resolves to a real provider");
}
