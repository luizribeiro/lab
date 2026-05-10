#![allow(dead_code)]
//! Shared lock-fixture helpers for c22+ V3 integration tests.

pub mod frontend_test_kit;
pub mod peer_test_kit;
pub mod session_test_kit;

#[cfg(feature = "test-fixture")]
pub mod m2_harness;

#[cfg(all(feature = "test-fixture", target_os = "linux"))]
pub mod fixture_smoke;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, LoadPolicy, Lock, LockFlags, PluginEntry, SessionTable,
};
use rafaello_core::manifest::SafePath;
use rafaello_core::validate::LockValidationContext;

pub fn canonical(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

pub fn entry(tools: &[&str], provider: bool, provider_id: Option<&str>) -> PluginEntry {
    let granted_at: DateTime<Utc> = "2026-01-15T08:30:00Z".parse().unwrap();
    PluginEntry {
        entry: SafePath::parse("bin/main.js").unwrap(),
        digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest_digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            .into(),
        granted_at,
        grant: Grant::default(),
        bindings: Bindings {
            provider,
            provider_id: provider_id.map(str::to_string),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            renderer_kinds: Vec::new(),
            tool_meta: BTreeMap::new(),
            load: LoadPolicy::default(),
        },
        flags: LockFlags::default(),
    }
}

pub fn entry_with_publishes(
    tools: &[&str],
    provider: bool,
    provider_id: Option<&str>,
    publishes: &[&str],
) -> PluginEntry {
    let mut e = entry(tools, provider, provider_id);
    e.grant.publishes = publishes.iter().map(|s| s.to_string()).collect();
    e
}

pub fn lock_with(plugins: Vec<(CanonicalId, PluginEntry)>, session: SessionTable) -> Lock {
    Lock {
        plugins: plugins.into_iter().collect(),
        session,
    }
}

/// Materialise `<base>/bin/main.js` so c34's entry-resolution
/// gate (the `entry` field defaulted by [`entry`]) passes.
pub fn make_plugin_dir(base: &Path) -> PathBuf {
    let bin = base.join("bin");
    std::fs::create_dir_all(&bin).expect("create plugin bin/");
    std::fs::write(bin.join("main.js"), b"// stub").expect("write entry");
    base.to_path_buf()
}

pub fn ctx_for(canonicals: &[&CanonicalId]) -> LockValidationContext {
    let mut plugin_dirs = BTreeMap::new();
    for c in canonicals {
        plugin_dirs.insert((*c).clone(), PathBuf::from(format!("/tmp/{}", c.name())));
    }
    LockValidationContext {
        project_root: PathBuf::from("/tmp/project"),
        home: PathBuf::from("/tmp/home"),
        plugin_dirs,
        cache_root: PathBuf::from("/tmp/cache"),
        state_root: PathBuf::from("/tmp/state"),
    }
}
