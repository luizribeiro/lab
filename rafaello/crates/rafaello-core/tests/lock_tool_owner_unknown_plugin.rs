//! c22 — `[session].tool_owner.grep = "github:nope/unknown@1.0.0"`
//! referencing an uninstalled plugin → `ToolOwnerUnknownPlugin`.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn tool_owner_unknown_plugin_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let b = canonical("github.com/other:beta@1.0.0");
    let mut tool_owner = BTreeMap::new();
    tool_owner.insert("grep".into(), "github.com/nope:ghost@1.0.0".into());
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
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ToolOwnerUnknownPlugin
    ));
}
