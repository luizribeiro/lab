//! c22 — `tool_owner.grep = "<plugin-A>"` when only plugin A claims
//! `"grep"` (no actual conflict) → `ToolOwnerRedundant`.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn tool_owner_without_conflict_is_redundant() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let mut tool_owner = BTreeMap::new();
    tool_owner.insert("grep".into(), a.to_string());
    let session = SessionTable {
        provider_active: None,
        tool_owner,
    };
    let lock = lock_with(vec![(a.clone(), entry(&["grep"], false, None))], session);
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ToolOwnerRedundant
    ));
}
