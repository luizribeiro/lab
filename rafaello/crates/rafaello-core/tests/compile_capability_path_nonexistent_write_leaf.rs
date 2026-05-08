//! c31 — §C3 resolver per pi review-5 finding 7: a non-existent
//! leaf inside an existing ancestor (`${project}/target/new`,
//! where `target/` exists but `target/new` does not) compiles
//! cleanly. The lexical-suffix step is what makes
//! `write_dirs = ["${project}/target/new"]` legitimate without
//! requiring the leaf to exist at compile time.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn nonexistent_write_leaf_compiles() {
    let tmp = tempfile::tempdir().unwrap();
    let project = fs::canonicalize(tmp.path()).unwrap();
    fs::create_dir(project.join("target")).unwrap();
    // intentionally do NOT create `target/new` — it's a write_dirs
    // leaf the plugin will mkdir at runtime.

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                write_dirs: vec!["${project}/target/new".to_owned()],
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

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");

    let leaf = project.join("target").join("new");
    assert!(plan.filesystem.write_dirs.contains(&leaf));
}
