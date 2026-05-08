//! c25 — V3 lock-side capability-path template re-validation (pi-6 finding 3).

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem, SessionTable};
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn lock_relative_read_dir_is_rejected() {
    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["relative/path".to_owned()],
                ..GrantFilesystem::default()
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockCapabilityPathRelative
    ));
}

#[test]
fn lock_relative_exec_path_is_rejected() {
    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                exec_paths: vec!["bin/tool".to_owned()],
                ..GrantFilesystem::default()
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);
    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::LockCapabilityPathRelative
    ));
}
