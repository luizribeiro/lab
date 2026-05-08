//! c27 — V3 lock-side `exec_paths` / `exec_dirs` under `${project}` refusal
//! (scope §V3 exec_paths bullet, security RFC §6.9).

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem, SessionTable};
use rafaello_core::validate::{self, LockValidationContext};

use common::{canonical, entry, lock_with};

#[test]
fn lock_exec_dir_inside_project_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().to_path_buf();

    let id = canonical("github.com/acme:alpha@1.0.0");
    let mut e = entry(&["alpha"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                exec_dirs: vec!["${project}/bin".to_owned()],
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

    let mut plugin_dirs = BTreeMap::new();
    plugin_dirs.insert(id.clone(), project_root.join(".rafaello/plugins/alpha"));
    let ctx = LockValidationContext {
        project_root,
        home: PathBuf::from("/tmp/home"),
        plugin_dirs,
        cache_root: PathBuf::from("/tmp/cache"),
        state_root: PathBuf::from("/tmp/state"),
    };

    assert!(matches!(
        validate::lock(&lock, &ctx).unwrap_err(),
        ValidationError::ExecPathInsideProject
    ));
}
