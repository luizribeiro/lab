//! c29 — `compile_plugin` precondition contract per §C1.1: a lock
//! that violates a V3 invariant, fed straight to `compile_plugin`
//! without a prior `validate::lock`, returns
//! `CompileError::ValidationNotRun`. Two installed plugins claim
//! the same tool name without a `session.tool_owner` resolution —
//! the V3-must-run-first guard fires.

mod common;

use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::SessionTable;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn unresolved_tool_conflict_without_v3_returns_validation_not_run() {
    let a = canonical("github.com/acme:alpha@1.0.0");
    let b = canonical("github.com/other:beta@1.0.0");
    let lock = lock_with(
        vec![
            (a.clone(), entry(&["grep"], false, None)),
            (b.clone(), entry(&["grep"], false, None)),
        ],
        SessionTable::default(),
    );

    let ctx = PathContext {
        project_root: PathBuf::from("/tmp/project"),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: PathBuf::from("/tmp/plugin/alpha"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let err = compile_plugin(&lock, &a, &ctx, &digests).unwrap_err();
    assert!(matches!(err, CompileError::ValidationNotRun));
}
