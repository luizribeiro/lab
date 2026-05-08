//! c31 — §C5 per-plugin private-state grant: the compiled plan
//! unconditionally contains
//! `${project}/.rafaello-plugin-data/<topic-id>/` in both
//! `read_dirs` and `write_dirs`, regardless of whether the lock
//! requested it. **Topic-id form** per pi review-2 finding 1 — the
//! raw canonical id is not a safe filename.

mod common;

use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::SessionTable;
use rafaello_core::paths::PathContext;
use rafaello_core::topic_id;

use common::{canonical, entry, lock_with};

#[test]
fn private_state_grant_present_in_both_directions() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let e = entry(&["writer"], false, None);
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

    let topic = topic_id::derive(&id.to_string());
    let expected = project.join(".rafaello-plugin-data").join(&topic);

    assert!(
        plan.filesystem.read_dirs.contains(&expected),
        "read_dirs missing private state {expected:?}: {:?}",
        plan.filesystem.read_dirs
    );
    assert!(
        plan.filesystem.write_dirs.contains(&expected),
        "write_dirs missing private state {expected:?}: {:?}",
        plan.filesystem.write_dirs
    );

    assert!(
        !topic.contains(':') && !topic.contains('@') && !topic.contains('/'),
        "topic-id {topic:?} should be filename-safe, not the raw canonical id"
    );
}
