//! c20 — with `allow_credential_paths = true`, broad grants
//! (read + write) over `${home}` compile verbatim and the
//! override flag surfaces in `DecomposedGrant.flags`.

use std::path::PathBuf;

use rafaello_core::carveout::compile_against;
use rafaello_core::lock::{CanonicalId, GrantBundle, GrantFilesystem};
use rafaello_core::paths::PathContext;

#[test]
fn override_emits_grants_verbatim_and_records_flag() {
    let ctx = PathContext {
        project_root: PathBuf::from("/work/proj"),
        home: PathBuf::from("/home/u"),
        plugin_dir: PathBuf::from("/work/proj/.rafaello/plugins/p"),
        cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
        state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
    };
    let bundle = GrantBundle {
        filesystem: Some(GrantFilesystem {
            read_dirs: vec!["${home}".to_owned()],
            write_dirs: vec!["${home}/scratch".to_owned()],
            read_paths: vec!["${home}/.netrc".to_owned()],
            ..GrantFilesystem::default()
        }),
        ..GrantBundle::default()
    };
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let out = compile_against(&bundle, &id, &ctx, true).unwrap();

    assert_eq!(out.read_dirs, vec![PathBuf::from("/home/u")]);
    assert_eq!(out.read_paths, vec![PathBuf::from("/home/u/.netrc")]);
    assert_eq!(out.write_dirs, vec![PathBuf::from("/home/u/scratch")]);
    assert!(out.flags.allow_credential_paths_active);
}
