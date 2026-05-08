//! c14 — `tool_meta.<n>.always_confirm` round-trips through
//! `Lock::to_toml` / `Lock::from_toml` byte-equal.
//!
//! Lock-side half of the two-stage test (m0 §4.3); the
//! `CompiledPlugin.tool_meta` half lands in c34. Constructed
//! programmatically per scope §"Out of scope" — m1 fixtures do
//! not project from manifests.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use rafaello_core::lock::{
    Bindings, CanonicalId, Lock, LockFlags, PluginEntry, ToolMeta,
};
use rafaello_core::lock::grant::Grant;
use rafaello_core::manifest::safepath::SafePath;

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
        session: Default::default(),
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
