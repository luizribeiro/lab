//! c22 — Two installed plugins both listing `"grep"` in `bindings.tools`
//! without a `[session].tool_owner.grep` decision → `ConflictingToolName`.
//! With `tool_owner.grep = "<plugin-A>"` the conflict resolves.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn unresolved_tool_conflict_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let b = canonical("github.com/other:beta@1.0.0");
    let lock = lock_with(
        vec![
            (a.clone(), entry(&["grep"], false, None)),
            (b.clone(), entry(&["grep"], false, None)),
        ],
        SessionTable::default(),
    );
    let ctx = ctx_for(&[&a, &b]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ConflictingToolName
    ));
}

#[test]
fn tool_owner_resolves_conflict() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let b = canonical("github.com/other:beta@1.0.0");
    let mut tool_owner = BTreeMap::new();
    tool_owner.insert("grep".into(), a.to_string());
    let session = SessionTable {
        provider_active: None,
        tool_owner,
    };
    let lock = lock_with(
        vec![
            (a.clone(), entry(&["grep"], false, None)),
            (b.clone(), entry(&["grep"], false, None)),
        ],
        session,
    );
    let ctx = ctx_for(&[&a, &b]);
    validate::lock(&lock, &ctx).expect("tool_owner resolves the conflict");
}
