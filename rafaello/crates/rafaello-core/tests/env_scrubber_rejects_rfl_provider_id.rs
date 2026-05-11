//! c05 — scope §PS5 + §M1.1: m1's compile-time scrubber rejects
//! `RFL_PROVIDER_ID` in both `env.pass` and `env.set` (mirror of
//! supervisor's m2 reserved list per scope §PS4).

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

fn compile_with_env(env: GrantEnv) -> CompileError {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            env: Some(env),
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

    compile_plugin(&lock, &id, &ctx, &digests).expect_err("must reject")
}

#[test]
fn rfl_provider_id_in_env_pass_is_rejected() {
    let err = compile_with_env(GrantEnv {
        pass: vec!["RFL_PROVIDER_ID".to_owned()],
        set: BTreeMap::new(),
        allow_secrets: Vec::new(),
    });
    assert!(
        matches!(err, CompileError::ReservedEnvVarRequested),
        "expected ReservedEnvVarRequested for env.pass, got {err:?}"
    );
}

#[test]
fn rfl_provider_id_in_env_set_is_rejected() {
    let err = compile_with_env(GrantEnv {
        pass: Vec::new(),
        set: BTreeMap::from([("RFL_PROVIDER_ID".to_owned(), "x".to_owned())]),
        allow_secrets: Vec::new(),
    });
    assert!(
        matches!(err, CompileError::ReservedEnvVarRequested),
        "expected ReservedEnvVarRequested for env.set, got {err:?}"
    );
}
