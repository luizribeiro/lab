//! c22 — `[session].provider_active` referencing a plugin not present
//! in the lock → `ValidationError::ProviderActiveUnknown`.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn provider_active_unknown_plugin_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let session = SessionTable {
        provider_active: Some("github.com/nope:ghost@1.0.0".into()),
        tool_owner: Default::default(),
    };
    let lock = lock_with(vec![(a.clone(), entry(&["alpha"], false, None))], session);
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ProviderActiveUnknown
    ));
}
