//! c26 — `bindings.tool_meta.foo` exists but `bindings.tools` does
//! not contain `"foo"` → `ValidationError::OrphanToolMeta`.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{SessionTable, ToolMeta};
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn tool_meta_without_matching_tool_is_orphan() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["grep"], false, None);
    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "foo".to_string(),
        ToolMeta {
            sinks: Vec::new(),
            sinks_inferred: false,
            grant_match: None,
            always_confirm: false,
        },
    );
    e.bindings.tool_meta = tool_meta;
    let lock = lock_with(vec![(a.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::OrphanToolMeta
    ));
}
