//! c32 — §C1 / Risks §2 negative: a lock with `network.mode =
//! "proxy"` and a syntactically-invalid `allow_hosts` entry is
//! rejected at compile time via the
//! `outpost::NetworkPolicy::from_allowed_hosts` parse-time dry-run,
//! returning `CompileError::InvalidAllowHosts`. The parsed policy
//! is discarded — m2 reconstructs at spawn (pi review-2 finding 6).

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::error::CompileError;
use rafaello_core::lock::{Grant, GrantBundle, GrantNetwork, SessionTable};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn invalid_allow_hosts_rejected_via_dry_run() {
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
                allow_hosts: vec!["not a hostname".to_owned()],
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

    let err = compile_plugin(&lock, &id, &ctx, &digests).expect_err("must reject");
    assert!(
        matches!(err, CompileError::InvalidAllowHosts),
        "expected InvalidAllowHosts, got {err:?}"
    );
}
