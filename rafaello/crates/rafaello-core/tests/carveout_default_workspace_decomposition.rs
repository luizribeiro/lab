//! c20 — default workspace grant `read_dirs = ["${project}"]`
//! decomposes into the immediate non-hidden children of the
//! project root (§K3).

use std::fs;
use std::path::PathBuf;

use rafaello_core::carveout::compile_against;
use rafaello_core::lock::{CanonicalId, GrantBundle, GrantFilesystem};
use rafaello_core::paths::PathContext;

fn ctx(project: PathBuf) -> PathContext {
    PathContext {
        home: PathBuf::from("/home/u"),
        plugin_dir: project.join(".rafaello/plugins/p"),
        cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
        state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
        project_root: project,
    }
}

#[test]
fn project_root_decomposes_to_non_hidden_children() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    fs::create_dir(project.join("src")).unwrap();
    fs::create_dir(project.join("docs")).unwrap();
    fs::write(project.join("README.md"), "x").unwrap();

    let bundle = GrantBundle {
        filesystem: Some(GrantFilesystem {
            read_dirs: vec!["${project}".to_owned()],
            ..GrantFilesystem::default()
        }),
        ..GrantBundle::default()
    };
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let out = compile_against(&bundle, &id, &ctx(project.clone()), false).unwrap();

    let mut expected = vec![
        project.join("README.md"),
        project.join("docs"),
        project.join("src"),
    ];
    expected.sort();
    assert_eq!(out.read_dirs, expected);
    assert!(!out.flags.allow_credential_paths_active);
}
