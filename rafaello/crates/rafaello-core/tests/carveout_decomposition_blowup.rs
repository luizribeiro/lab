//! c20 — a project root with 300 immediate non-hidden children
//! plus `.rafaello/` exceeds the 256-entry decomposition cap →
//! `CompileError::CarveOutTooLarge` (security RFC §7.3 rule 4).

use std::fs;
use std::path::PathBuf;

use rafaello_core::carveout::compile_against;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{CanonicalId, GrantBundle, GrantFilesystem};
use rafaello_core::paths::PathContext;

#[test]
fn decomposition_over_cap_is_too_large() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    fs::create_dir(project.join(".rafaello")).unwrap();
    for i in 0..300 {
        fs::write(project.join(format!("f{i:03}")), "x").unwrap();
    }

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
    let err = compile_against(&bundle, &id, &ctx, false).unwrap_err();
    assert!(matches!(err, CompileError::CarveOutTooLarge));
}
