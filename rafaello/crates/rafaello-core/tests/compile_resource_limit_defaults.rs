//! c34 — §C6 resource-limit defaults: a lock that omits `limits`
//! compiles to `max_cpu_time = 300`, `max_open_files = 1024`. An
//! explicit `0` (provider-plugin shape per manifest RFC §9.3) is
//! preserved verbatim and **not** overridden by the default.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantLimits, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

fn ctx(project: &std::path::Path, plugin: &str) -> PathContext {
    PathContext {
        project_root: project.to_path_buf(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: project.join(".rafaello/plugins").join(plugin),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    }
}

fn digests() -> RecomputedDigests {
    RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    }
}

#[test]
fn omitted_limits_default_to_300_cpu_1024_fds() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let e = entry(&["writer"], false, None);
    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx(&project, "writer");
    common::make_plugin_dir(&ctx.plugin_dir);

    let plan = compile_plugin(&lock, &id, &ctx, &digests()).expect("compile succeeds");
    assert_eq!(plan.limits.max_cpu_time, 300);
    assert_eq!(plan.limits.max_open_files, 1024);
    assert_eq!(plan.limits.max_address_space, None);
    assert_eq!(plan.limits.max_processes, None);
}

#[test]
fn explicit_zero_cpu_preserves_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:provider@1.0.0");
    let mut e = entry(&[], true, Some("acme"));

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            limits: Some(GrantLimits {
                max_cpu_time: Some(0),
                max_open_files: Some(0),
                max_address_space: None,
                max_processes: None,
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx(&project, "provider");
    common::make_plugin_dir(&ctx.plugin_dir);

    let plan = compile_plugin(&lock, &id, &ctx, &digests()).expect("compile succeeds");
    assert_eq!(plan.limits.max_cpu_time, 0);
    assert_eq!(plan.limits.max_open_files, 0);
}
