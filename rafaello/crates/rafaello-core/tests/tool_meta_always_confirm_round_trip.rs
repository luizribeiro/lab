//! c14 + c34 — `tool_meta.<n>.always_confirm` round-trips through
//! `Lock::to_toml` / `Lock::from_toml` byte-equal (c14 lock-side
//! half) **and** survives projection into
//! `CompiledPlugin.tool_meta` (c34 compile-side half — closes the
//! m0 §4.3 two-stage test). Constructed programmatically per
//! scope §"Out of scope" — m1 fixtures do not project from
//! manifests.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{
    Bindings, CanonicalId, Lock, LockFlags, PluginEntry, SessionTable, ToolMeta,
};
use rafaello_core::lock::grant::Grant;
use rafaello_core::manifest::safepath::SafePath;
use rafaello_core::paths::PathContext;

fn fixture_lock() -> Lock {
    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "grep".to_owned(),
        ToolMeta {
            sinks: vec!["workspace_write".to_owned()],
            sinks_inferred: false,
            grant_match: None,
            always_confirm: true,
        },
    );

    let bindings = Bindings {
        provider: false,
        provider_id: None,
        tools: vec!["grep".to_owned()],
        renderer_kinds: Vec::new(),
        tool_meta,
        load: Default::default(),
    };

    let entry = PluginEntry {
        entry: SafePath::parse("bin/grep.js").expect("safepath"),
        digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_owned(),
        manifest_digest:
            "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
                .to_owned(),
        granted_at: Utc.with_ymd_and_hms(2026, 1, 15, 8, 30, 0).unwrap(),
        grant: Grant::default(),
        bindings,
        flags: LockFlags::default(),
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(
        CanonicalId::parse("github.com/acme:grep@1.4.2").expect("canonical id"),
        entry,
    );

    Lock {
        plugins,
        session: SessionTable::default(),
    }
}

#[test]
fn always_confirm_survives_toml_round_trip() {
    let lock = fixture_lock();

    let serialised = lock.to_toml();
    let parsed = Lock::from_toml(&serialised).expect("parse");

    assert_eq!(lock, parsed, "Lock value round-trips through TOML");

    let reserialised = parsed.to_toml();
    assert_eq!(
        serialised, reserialised,
        "TOML output is byte-equal across a re-serialise"
    );

    let id = CanonicalId::parse("github.com/acme:grep@1.4.2").unwrap();
    let meta = parsed
        .plugins
        .get(&id)
        .expect("plugin entry present")
        .bindings
        .tool_meta
        .get("grep")
        .expect("tool_meta.grep present");
    assert!(
        meta.always_confirm,
        "always_confirm survives the round-trip"
    );
}

#[test]
fn always_confirm_projects_through_compile_plugin() {
    let lock = fixture_lock();
    let id = CanonicalId::parse("github.com/acme:grep@1.4.2").unwrap();
    let entry = lock.plugins.get(&id).unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();
    let plugin_dir = project.join(".rafaello/plugins/grep");
    std::fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    std::fs::write(plugin_dir.join("bin/grep.js"), b"// stub").unwrap();

    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir,
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    let digests = RecomputedDigests {
        content: entry.digest.clone(),
        manifest: entry.manifest_digest.clone(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile succeeds");
    let projected = plan
        .tool_meta
        .get("grep")
        .expect("tool_meta.grep present in compiled plan");
    assert!(
        projected.always_confirm,
        "always_confirm carried through CompiledPlugin.tool_meta"
    );
    assert_eq!(projected.sinks, vec!["workspace_write".to_owned()]);
}
