//! c31 — §C3 negative: a capability path containing an unknown
//! placeholder (`${nope}`) is rejected with
//! `CompileError::UnknownPlaceholder`.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn unknown_placeholder_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let project = fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${nope}/src".to_owned()],
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
    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: project.join(".rafaello/plugins/writer"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let err = compile_plugin(&lock, &id, &ctx, &digests).unwrap_err();
    assert!(
        matches!(err, CompileError::UnknownPlaceholder),
        "expected UnknownPlaceholder, got {err:?}"
    );
}
