//! c24 — V3 lock-side publish ACL: `plugin.<id>.*` whose `<id>`
//! does not match `topic_id::derive(canonical)` is rejected.

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry_with_publishes, lock_with};

#[test]
fn lock_publish_on_foreign_topic_id_is_rejected() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let plug = entry_with_publishes(&["alpha"], false, None, &["plugin.id_aaaaaaaaaaaaaaaa.foo"]);
    let lock = lock_with(vec![(a.clone(), plug)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockPublishOnForeignTopicId
    ));
}
