//! c20 — explicit leaf hits on either class refuse with
//! `CarveOutRefused` (no silent drop, pi-2 finding 7), and the
//! `allow_credential_paths` override compiles them verbatim for
//! both classes.

use std::path::PathBuf;

use rafaello_core::carveout::compile_against;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{CanonicalId, GrantBundle, GrantFilesystem};
use rafaello_core::paths::PathContext;

fn ctx() -> PathContext {
    PathContext {
        project_root: PathBuf::from("/work/proj"),
        home: PathBuf::from("/home/u"),
        plugin_dir: PathBuf::from("/work/proj/.rafaello/plugins/p"),
        cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
        state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
    }
}

fn bundle_with_read_path(p: &str) -> GrantBundle {
    GrantBundle {
        filesystem: Some(GrantFilesystem {
            read_paths: vec![p.to_owned()],
            ..GrantFilesystem::default()
        }),
        ..GrantBundle::default()
    }
}

#[test]
fn project_lockfile_explicit_read_path_refused() {
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let err = compile_against(
        &bundle_with_read_path("${project}/rafaello.lock"),
        &id,
        &ctx(),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, CompileError::CarveOutRefused));
}

#[test]
fn netrc_explicit_read_path_refused() {
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let err =
        compile_against(&bundle_with_read_path("${home}/.netrc"), &id, &ctx(), false).unwrap_err();
    assert!(matches!(err, CompileError::CarveOutRefused));
}

#[test]
fn override_compiles_explicit_leaf_hits() {
    let id = CanonicalId::parse("github.com/acme:tool@1.0.0").unwrap();
    let lock = compile_against(
        &bundle_with_read_path("${project}/rafaello.lock"),
        &id,
        &ctx(),
        true,
    )
    .unwrap();
    assert_eq!(
        lock.read_paths,
        vec![PathBuf::from("/work/proj/rafaello.lock")]
    );
    assert!(lock.flags.allow_credential_paths_active);

    let netrc =
        compile_against(&bundle_with_read_path("${home}/.netrc"), &id, &ctx(), true).unwrap();
    assert_eq!(netrc.read_paths, vec![PathBuf::from("/home/u/.netrc")]);
    assert!(netrc.flags.allow_credential_paths_active);
}
