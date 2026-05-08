//! c20 — `read_dirs = ["${project}"]` against a project that
//! contains `.rafaello/` decomposes around the project-class
//! carve-out: the result has no entry covering `.rafaello/`
//! (filtered by the §K3 hidden-directory rule).

use std::fs;
use std::path::PathBuf;

use rafaello_core::carveout::compile_against;
use rafaello_core::lock::{CanonicalId, GrantBundle, GrantFilesystem};
use rafaello_core::paths::PathContext;

#[test]
fn rafaello_dot_dir_excluded_from_decomposition() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    fs::create_dir(project.join(".rafaello")).unwrap();
    fs::create_dir(project.join("src")).unwrap();
    fs::write(project.join("rafaello.lock"), "x").unwrap();

    let ctx = PathContext {
        home: PathBuf::from("/home/u"),
        plugin_dir: project.join(".rafaello/plugins/p"),
        cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
        state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
        project_root: project.clone(),
    };
    let bundle = GrantBundle {
        filesystem: Some(GrantFilesystem {
            read_dirs: vec!["${project}".to_owned()],
            ..GrantFilesystem::default()
        }),
        ..GrantBundle::default()
    };
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let out = compile_against(&bundle, &id, &ctx, false).unwrap();

    assert_eq!(out.read_dirs, vec![project.join("src")]);
    assert!(!out
        .read_dirs
        .iter()
        .any(|p| p.starts_with(project.join(".rafaello"))));
    assert!(!out.read_dirs.contains(&project.join("rafaello.lock")));
}
