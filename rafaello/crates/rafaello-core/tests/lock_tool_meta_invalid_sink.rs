//! c26 — `bindings.tool_meta.<n>.sinks = ["Network"]` (uppercase,
//! fails sink-class grammar) rejected by V3.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{SessionTable, ToolMeta};
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn lock_tool_meta_invalid_sink_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["grep"], false, None);
    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "grep".to_string(),
        ToolMeta {
            sinks: vec!["Network".to_string()],
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
        ValidationError::IllegalSinkClass { .. }
    ));
}
