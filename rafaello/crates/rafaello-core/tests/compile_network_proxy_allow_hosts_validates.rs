//! c32 — §C1 NetworkPlan dry-run: the compile-time
//! `outpost::NetworkPolicy::from_allowed_hosts(...)` call accepts
//! the worked-example proxy `allow_hosts` list. The parsed policy
//! is discarded; m1 emits only `NetworkPlan::Proxy { allow_hosts }`
//! with the list verbatim (Risks §2 / pi review-2 finding 6).

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::{compile_plugin, NetworkPlan};
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantNetwork, SessionTable};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn proxy_allow_hosts_dry_run_accepts_worked_example() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["api.example.com".to_owned(), "*.example.com".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: project.join(".rafaello/plugins/writer"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    common::make_plugin_dir(&ctx.plugin_dir);
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests)
        .expect("dry-run validation accepts worked-example allow_hosts");

    assert!(matches!(plan.network, NetworkPlan::Proxy { .. }));
}
