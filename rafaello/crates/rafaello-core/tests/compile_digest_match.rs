//! c34 — §D3 digest gating positive: when the recomputed
//! `content` and `manifest` digests match the lock's `digest` and
//! `manifest_digest` fields, `compile_plugin` succeeds. The lock
//! fixture's `entry` is materialised under `plugin_dir` so the
//! c34 entry-resolution gate also passes.

mod common;

use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::SessionTable;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn matching_content_and_manifest_digests_compile_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let e = entry(&["writer"], false, None);
    let lock = lock_with(vec![(id.clone(), e.clone())], SessionTable::default());

    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: project.join(".rafaello/plugins/writer"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    common::make_plugin_dir(&ctx.plugin_dir);

    let digests = RecomputedDigests {
        content: e.digest.clone(),
        manifest: e.manifest_digest.clone(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");
    let expected_entry = ctx.plugin_dir.join("bin/main.js");
    let expected_canon = std::fs::canonicalize(&expected_entry).unwrap();
    assert_eq!(plan.entry_absolute, expected_canon);
}
