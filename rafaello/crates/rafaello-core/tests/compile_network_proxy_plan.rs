//! c32 — §C1 NetworkPlan emission: a lock with `network.mode =
//! "proxy"` and a non-empty `allow_hosts` list compiles to
//! `NetworkPlan::Proxy { allow_hosts }` recording the host list
//! verbatim. m2 starts the outpost proxy at spawn time; m1 only
//! emits the plan.

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
fn proxy_plan_records_allow_hosts_verbatim() {
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
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");
    match plan.network {
        NetworkPlan::Proxy { allow_hosts } => {
            let mut expected = vec!["*.example.com".to_owned(), "api.example.com".to_owned()];
            expected.sort();
            assert_eq!(allow_hosts, expected);
        }
        other => panic!("expected Proxy plan, got {other:?}"),
    }
}
