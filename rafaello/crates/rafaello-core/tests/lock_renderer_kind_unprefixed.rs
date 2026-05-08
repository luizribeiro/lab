//! c26 — `bindings.renderer_kinds = ["code.diff"]` (unprefixed plugin
//! kind) rejected by V3's M7 mirror.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn unprefixed_renderer_kind_in_lock_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&[], false, None);
    e.bindings.renderer_kinds = vec!["code.diff".to_string()];
    let lock = lock_with(vec![(a.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::UnprefixedRendererKind { .. }
    ));
}
