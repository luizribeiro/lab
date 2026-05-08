//! c32 — §C7.1 negative: a lock that requests `RFL_BUS_FD` /
//! `RFL_PLUGIN` in `env.pass` is rejected at compile time with
//! `CompileError::ReservedEnvVarRequested`. Per pi review-2 finding
//! 2 / pi review-3 finding 8 the compiler invokes
//! `scrubber::reject_reserved` *before* secret-pattern stripping
//! so reserved-var requests are loud rather than silently dropped.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn rfl_bus_fd_in_env_pass_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            env: Some(GrantEnv {
                pass: vec!["RFL_BUS_FD".to_owned()],
                set: BTreeMap::new(),
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
    common::make_plugin_dir(&ctx.plugin_dir);
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let err = compile_plugin(&lock, &id, &ctx, &digests).expect_err("must reject");
    assert!(
        matches!(err, CompileError::ReservedEnvVarRequested),
        "expected ReservedEnvVarRequested, got {err:?}"
    );
}
