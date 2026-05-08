//! c34 — §D3 digest gating negative: when the recomputed content
//! digest does not match `lock.digest`, `compile_plugin` fails
//! with `CompileError::ContentDigestMismatch`.

mod common;

use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::SessionTable;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn content_digest_mismatch_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let lock = lock_with(
        vec![(id.clone(), entry(&["writer"], false, None))],
        SessionTable::default(),
    );
    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: project.join(".rafaello/plugins/writer"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    common::make_plugin_dir(&ctx.plugin_dir);

    let digests = RecomputedDigests {
        content: "sha256:dead00000000000000000000000000000000000000000000000000000000beef".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let err = compile_plugin(&lock, &id, &ctx, &digests).unwrap_err();
    assert!(
        matches!(err, CompileError::ContentDigestMismatch),
        "got {err:?}"
    );
}
