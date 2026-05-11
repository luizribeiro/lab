//! c07 — V3 lock-side publish ACL: a typo'd plural like
//! `frontends.tui.x` is rejected with `PublishUnknownNamespace`
//! (scope §M1.2).

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry_with_publishes, lock_with};

#[test]
fn lock_publish_on_evil_top_segment_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let plug = entry_with_publishes(&["alpha"], false, None, &["frontends.tui.x"]);
    let lock = lock_with(vec![(a.clone(), plug)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    match validate::lock(&lock, &ctx).unwrap_err() {
        ValidationError::PublishUnknownNamespace { topic, top } => {
            assert_eq!(topic, "frontends.tui.x");
            assert_eq!(top, "frontends");
        }
        other => panic!("expected PublishUnknownNamespace, got {other:?}"),
    }
}
