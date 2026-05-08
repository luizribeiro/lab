//! c34 — §L2 entry resolution: a symlink at `<plugin_dir>/bin/main.js`
//! whose canonical target lives outside `plugin_dir` is rejected
//! with `CompileError::EntryEscape`.

mod common;

use std::os::unix::fs as unix_fs;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::SessionTable;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn entry_symlink_escaping_plugin_dir_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let project = root.join("project");
    std::fs::create_dir(&project).unwrap();
    let plugin_dir = project.join(".rafaello/plugins/writer");
    std::fs::create_dir_all(plugin_dir.join("bin")).unwrap();

    let outside = root.join("outside.js");
    std::fs::write(&outside, b"// outside the package").unwrap();
    unix_fs::symlink(&outside, plugin_dir.join("bin/main.js")).unwrap();

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
    assert!(matches!(err, CompileError::EntryEscape), "got {err:?}");
}
