//! c31 — §C5 + §Tr4 second-stage assertion (m0 two-stage pattern).
//! `trifecta::evaluate` reads the lock's grant directly; the
//! private-state subtree is injected later by the compiler, so a
//! lock with no `write_dirs` reports `has_workspace_write == false`
//! even after `compile_plugin` injects the private-state grant.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem, SessionTable};
use rafaello_core::paths::PathContext;
use rafaello_core::topic_id;
use rafaello_core::trifecta;

use common::{canonical, entry, lock_with};

#[test]
fn compiler_injection_does_not_make_lock_a_workspace_writer() {
    let tmp = tempfile::tempdir().unwrap();
    let project = fs::canonicalize(tmp.path()).unwrap();
    fs::create_dir(project.join("src")).unwrap();

    let id = canonical("github.com/acme:reader@1.0.0");
    let mut e = entry(&["reader"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${project}/src".to_owned()],
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
        plugin_dir: project.join(".rafaello/plugins/reader"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");

    let topic = topic_id::derive(&id.to_string());
    let private_state = project.join(".rafaello-plugin-data").join(&topic);
    assert!(plan.filesystem.write_dirs.contains(&private_state));

    let state = trifecta::evaluate(&lock, &id, &ctx);
    assert!(
        !state.has_workspace_write,
        "private-state injection must not flip has_workspace_write: {state:?}"
    );
    assert!(!state.refuse);
}
