//! c31 — §C3 placeholder resolution: every capability path string
//! in the compiled plan is absolute and contains no `${...}`.

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
fn all_paths_in_plan_are_absolute_with_no_placeholders() {
    let tmp = tempfile::tempdir().unwrap();
    let project = fs::canonicalize(tmp.path()).unwrap();
    fs::create_dir(project.join("src")).unwrap();
    fs::create_dir(project.join("out")).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${project}/src".to_owned()],
                write_dirs: vec!["${project}/out".to_owned()],
                exec_paths: vec!["/usr/bin/ls".to_owned()],
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

    let all_paths = plan
        .filesystem
        .read_paths
        .iter()
        .chain(&plan.filesystem.read_dirs)
        .chain(&plan.filesystem.write_paths)
        .chain(&plan.filesystem.write_dirs)
        .chain(&plan.filesystem.exec_paths)
        .chain(&plan.filesystem.exec_dirs);

    for p in all_paths {
        assert!(p.is_absolute(), "path is not absolute: {p:?}");
        let s = p.to_string_lossy();
        assert!(
            !s.contains("${"),
            "unresolved placeholder remains in path: {s}"
        );
    }
}
