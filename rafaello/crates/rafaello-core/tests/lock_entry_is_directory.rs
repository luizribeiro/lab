//! c34 — §L2 entry resolution: when `plugin_dir/<entry>` resolves
//! to a directory rather than a regular file →
//! `CompileError::EntryNotFile`.

mod common;

use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::SessionTable;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn entry_pointing_at_directory_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();
    let plugin_dir = project.join(".rafaello/plugins/writer");
    std::fs::create_dir_all(plugin_dir.join("bin/main.js")).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let lock = lock_with(
        vec![(id.clone(), entry(&["writer"], false, None))],
        SessionTable::default(),
    );
    let ctx = PathContext {
        project_root: project,
        home: PathBuf::from("/tmp/home"),
        plugin_dir,
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let err = compile_plugin(&lock, &id, &ctx, &digests).unwrap_err();
    assert!(matches!(err, CompileError::EntryNotFile), "got {err:?}");
}
