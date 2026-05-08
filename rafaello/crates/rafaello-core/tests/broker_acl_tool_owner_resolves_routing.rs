//! c35 — Two plugins both claim `"grep"`;
//! `[session].tool_owner.grep = "<plugin-A>"`. `broker_acl::compile`'s
//! `tool_routes["grep"]` resolves to plugin A. The losing
//! plugin's `compile_plugin` output also has `"grep"` filtered
//! out of its `tool_meta` (G1, C1 — pi review-2 finding 4).

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rafaello_core::broker_acl;
use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{SessionTable, ToolMeta as LockToolMeta};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn tool_owner_routes_to_winner_and_filters_loser_tool_meta() {
    let id_a = canonical("github.com/acme:alpha@1.0.0");
    let id_b = canonical("github.com/other:beta@1.0.0");

    let a = entry(&["grep"], false, None);

    let mut b = entry(&["grep"], false, None);
    b.bindings.tool_meta.insert(
        "grep".to_owned(),
        LockToolMeta {
            sinks: vec!["workspace_read".to_owned()],
            sinks_inferred: false,
            grant_match: None,
            always_confirm: false,
        },
    );

    let mut tool_owner = BTreeMap::new();
    tool_owner.insert("grep".to_owned(), id_a.to_string());
    let session = SessionTable {
        provider_active: None,
        tool_owner,
    };

    let lock = lock_with(vec![(id_a.clone(), a), (id_b.clone(), b)], session);

    let acl = broker_acl::compile(&lock).expect("broker_acl::compile succeeds");
    assert_eq!(acl.tool_routes.get("grep"), Some(&id_a));

    let tmp = tempfile::tempdir().unwrap();
    let project = fs::canonicalize(tmp.path()).unwrap();
    let plugin_dir = project.join(".rafaello/plugins/beta");
    common::make_plugin_dir(&plugin_dir);

    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir,
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan_b = compile_plugin(&lock, &id_b, &ctx, &digests).expect("compile_plugin B succeeds");
    assert!(
        !plan_b.tool_meta.contains_key("grep"),
        "loser plugin must not project tool_meta for `grep`"
    );
}
