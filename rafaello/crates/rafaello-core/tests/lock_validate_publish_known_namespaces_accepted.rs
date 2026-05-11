//! c07 — V3 lock-side publish ACL: topics whose top-level segment
//! is in `{core, provider, plugin, frontend}` are *not* rejected
//! by the new unknown-namespace check; their existing shape rules
//! (`LockPublishOnReservedNamespace`, `LockPublishOnFrontendNamespace`,
//! `LockPublishOnForeignTopicId`, `LockProviderNamespaceMismatch`)
//! continue to govern acceptance (scope §M1.2).

mod common;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::SessionTable;
use rafaello_core::validate;

use common::{canonical, ctx_for, entry_with_publishes, lock_with};

fn validate_one(publish: &str) -> Result<(), ValidationError> {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let plug = entry_with_publishes(&["alpha"], false, None, &[publish]);
    let lock = lock_with(vec![(a.clone(), plug)], SessionTable::default());
    let ctx = ctx_for(&[&a]);
    validate::lock(&lock, &ctx)
}

#[test]
fn lock_publish_known_namespaces_skip_unknown_check() {
    for topic in [
        "core.x",
        "provider.foo.x",
        "plugin.id_abc.x",
        "frontend.tui.x",
    ] {
        if let Err(ValidationError::PublishUnknownNamespace { .. }) = validate_one(topic) {
            panic!("known top-level segment `{topic}` must not trip PublishUnknownNamespace");
        }
    }
}
