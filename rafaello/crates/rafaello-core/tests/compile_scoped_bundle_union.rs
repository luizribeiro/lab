//! c30 — full bundle union per `decisions.md` row 17 + pi review-4
//! finding 1: a `default` bundle plus a named `format` bundle each
//! contribute distinct authority; the compiled plan is the union
//! of both, with C4 post-flatten ordering (sort + dedup) applied.
//! There is no `active_bundles` selection knob — the spawn-time
//! policy reflects every named bundle.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::{compile_plugin, NetworkPlan};
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{
    Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantLimits, GrantNetwork, SessionTable,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn default_and_named_bundle_union() {
    let id = canonical("github.com/acme:formatter@1.0.0");
    let mut e = entry(&["format"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${project}/src".to_owned()],
                write_dirs: vec!["${project}/cache".to_owned()],
                ..GrantFilesystem::default()
            }),
            network: Some(GrantNetwork {
                mode: NetworkMode::Deny,
                allow_hosts: Vec::new(),
            }),
            env: Some(GrantEnv {
                pass: vec!["HOME".to_owned()],
                set: BTreeMap::from([("DEFAULT_KEY".to_owned(), "default".to_owned())]),
            }),
            limits: Some(GrantLimits {
                max_cpu_time: Some(60),
                max_open_files: None,
                max_address_space: None,
                max_processes: None,
            }),
        },
    );
    bundles.insert(
        "format".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec!["${project}/src".to_owned()],
                write_dirs: vec![
                    "${project}/out".to_owned(),
                    "${project}/dist".to_owned(),
                ],
                ..GrantFilesystem::default()
            }),
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["registry.example.com".to_owned()],
            }),
            env: Some(GrantEnv {
                pass: vec!["PATH".to_owned()],
                set: BTreeMap::from([("FORMAT_KEY".to_owned(), "fmt".to_owned())]),
            }),
            limits: Some(GrantLimits {
                max_cpu_time: Some(180),
                max_open_files: Some(1024),
                max_address_space: None,
                max_processes: None,
            }),
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = PathContext {
        project_root: PathBuf::from("/tmp/project"),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: PathBuf::from("/tmp/plugin/formatter"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");

    // Filesystem: union; duplicate `${project}/src` deduped; sorted.
    assert_eq!(
        plan.filesystem.read_dirs,
        vec![PathBuf::from("${project}/src")]
    );
    assert_eq!(
        plan.filesystem.write_dirs,
        vec![
            PathBuf::from("${project}/cache"),
            PathBuf::from("${project}/dist"),
            PathBuf::from("${project}/out"),
        ],
        "default's cache + format's dist/out unioned and sorted"
    );

    // Network: most-permissive (Proxy beats Deny); allow_hosts unioned.
    match plan.network {
        NetworkPlan::Proxy { allow_hosts } => {
            assert_eq!(allow_hosts, vec!["registry.example.com".to_owned()]);
        }
        other => panic!("expected Proxy plan, got {other:?}"),
    }

    // Env: pass unioned + sorted; set merged.
    assert_eq!(plan.env.pass, vec!["HOME".to_owned(), "PATH".to_owned()]);
    assert_eq!(
        plan.env.set,
        BTreeMap::from([
            ("DEFAULT_KEY".to_owned(), "default".to_owned()),
            ("FORMAT_KEY".to_owned(), "fmt".to_owned()),
        ])
    );

    // Limits: per-field max of present values.
    assert_eq!(plan.limits.max_cpu_time, 180);
    assert_eq!(plan.limits.max_open_files, 1024);
    assert_eq!(plan.limits.max_address_space, None);
    assert_eq!(plan.limits.max_processes, None);
}
