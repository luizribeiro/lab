//! c32 — §C1 EnvPlan emission: a lock with non-reserved `env.pass`
//! / `env.set` entries that don't trip the v1 secret-pattern globs
//! flows through to the compiled `EnvPlan` unchanged. Reserved-env
//! enforcement (§C7.1) and secret scrubbing (§Sc2) are exercised
//! separately.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn env_set_and_pass_pass_through_without_reserved_or_secret_hits() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            env: Some(GrantEnv {
                pass: vec!["HOME".to_owned(), "PATH".to_owned()],
                set: BTreeMap::from([
                    ("RUST_LOG".to_owned(), "info".to_owned()),
                    ("LANG".to_owned(), "en_US.UTF-8".to_owned()),
                ]),
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

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");
    assert_eq!(plan.env.pass, vec!["HOME".to_owned(), "PATH".to_owned()]);
    assert_eq!(
        plan.env.set,
        BTreeMap::from([
            ("LANG".to_owned(), "en_US.UTF-8".to_owned()),
            ("RUST_LOG".to_owned(), "info".to_owned()),
        ])
    );
}
