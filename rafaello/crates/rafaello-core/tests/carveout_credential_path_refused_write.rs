//! c20 — `write_dirs = ["${home}"]` covers `~/.ssh` without
//! `allow_credential_paths` → §K2 refusal (writes are never
//! decomposed in v1).

use std::path::PathBuf;

use rafaello_core::carveout::compile_against;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{CanonicalId, GrantBundle, GrantFilesystem};
use rafaello_core::paths::PathContext;

#[test]
fn home_write_dir_refused_for_credential_carveouts() {
    let ctx = PathContext {
        project_root: PathBuf::from("/work/proj"),
        home: PathBuf::from("/home/u"),
        plugin_dir: PathBuf::from("/work/proj/.rafaello/plugins/p"),
        cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
        state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
    };
    let bundle = GrantBundle {
        filesystem: Some(GrantFilesystem {
            write_dirs: vec!["${home}".to_owned()],
            ..GrantFilesystem::default()
        }),
        ..GrantBundle::default()
    };
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let err = compile_against(&bundle, &id, &ctx, false).unwrap_err();
    assert!(matches!(err, CompileError::CarveOutRefused));
}
