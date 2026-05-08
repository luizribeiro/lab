//! c30 — bundle flatten with only the `default` bundle: the
//! resulting `CompiledPlugin`'s effective filesystem / network /
//! env / limits mirror the lone bundle's values, with C4
//! post-flatten ordering (sort + dedup) applied. Capability path
//! strings are still raw — placeholder substitution is c31.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rafaello_core::compile::{compile_plugin, NetworkPlan};
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{
    Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantLimits, GrantNetwork, SessionTable,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn default_bundle_flattens_into_compiled_plugin() {
    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_dirs: vec![
                    "${project}/src".to_owned(),
                    "${project}/docs".to_owned(),
                    "${project}/src".to_owned(),
                ],
                write_dirs: vec!["${project}/out".to_owned()],
                exec_paths: vec!["/usr/bin/ls".to_owned()],
                ..GrantFilesystem::default()
            }),
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned(), "cdn.example.com".to_owned()],
            }),
            env: Some(GrantEnv {
                pass: vec!["HOME".to_owned(), "PATH".to_owned(), "HOME".to_owned()],
                set: BTreeMap::from([("RUST_LOG".to_owned(), "info".to_owned())]),
            }),
            limits: Some(GrantLimits {
                max_cpu_time: Some(120),
                max_open_files: Some(2048),
                max_address_space: None,
                max_processes: Some(64),
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
        plugin_dir: PathBuf::from("/tmp/plugin/writer"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");

    assert_eq!(
        plan.filesystem.read_dirs,
        vec![
            PathBuf::from("${project}/docs"),
            PathBuf::from("${project}/src"),
        ],
        "read_dirs sorted + deduped per C4"
    );
    assert_eq!(
        plan.filesystem.write_dirs,
        vec![PathBuf::from("${project}/out")]
    );
    assert_eq!(
        plan.filesystem.exec_paths,
        vec![PathBuf::from("/usr/bin/ls")]
    );
    assert!(plan.filesystem.read_paths.is_empty());
    assert!(plan.filesystem.write_paths.is_empty());
    assert!(plan.filesystem.exec_dirs.is_empty());

    match plan.network {
        NetworkPlan::Proxy { allow_hosts } => {
            assert_eq!(
                allow_hosts,
                vec!["api.example.com".to_owned(), "cdn.example.com".to_owned()]
            );
        }
        other => panic!("expected Proxy plan, got {other:?}"),
    }

    assert_eq!(plan.env.pass, vec!["HOME".to_owned(), "PATH".to_owned()]);
    assert_eq!(
        plan.env.set,
        BTreeMap::from([("RUST_LOG".to_owned(), "info".to_owned())])
    );

    assert_eq!(plan.limits.max_cpu_time, 120);
    assert_eq!(plan.limits.max_open_files, 2048);
    assert_eq!(plan.limits.max_address_space, None);
    assert_eq!(plan.limits.max_processes, Some(64));

    // Path resolver hook-in is c31 — strings remain unresolved here.
    let raw: &Path = Path::new("${project}/src");
    assert!(plan.filesystem.read_dirs.contains(&raw.to_path_buf()));
}
