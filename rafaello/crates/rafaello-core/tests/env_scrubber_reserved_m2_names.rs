//! c04 — m2 §F5 / §SP4.4: hand-mutated locks that put any of the
//! four m2-reserved env names in `env.set` are rejected at compile
//! time, just like m1's `RFL_BUS_FD` / `RFL_PLUGIN`.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

const M2_RESERVED_NAMES: &[&str] = &[
    "RFL_HELPER_FD",
    "RFL_TOPIC_ID",
    "RFL_PROJECT_ROOT",
    "RFL_PRIVATE_STATE_DIR",
];

#[test]
fn m2_reserved_names_in_env_set_are_rejected() {
    for name in M2_RESERVED_NAMES {
        let tmp = tempfile::tempdir().unwrap();
        let project = std::fs::canonicalize(tmp.path()).unwrap();

        let id = canonical("github.com/acme:writer@1.0.0");
        let mut e = entry(&["writer"], false, None);

        let mut bundles = BTreeMap::new();
        bundles.insert(
            "default".to_owned(),
            GrantBundle {
                env: Some(GrantEnv {
                    pass: Vec::new(),
                    set: BTreeMap::from([((*name).to_owned(), "x".to_owned())]),
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
            content: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .into(),
            manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .into(),
        };

        let err = compile_plugin(&lock, &id, &ctx, &digests).expect_err("must reject");
        assert!(
            matches!(err, CompileError::ReservedEnvVarRequested),
            "expected ReservedEnvVarRequested for {name}, got {err:?}"
        );
    }
}
