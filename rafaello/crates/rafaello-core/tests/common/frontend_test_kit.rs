#![allow(dead_code)]
//! Shared builders for c18+ frontend Phase A negative tests.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::EnvPlan;
use rafaello_core::frontend::{CompiledFrontend, FrontendPaths};
use rafaello_core::lock::canonical_id::CanonicalId;
use tempfile::TempDir;

pub const KNOWN_ATTACH_ID: &str = "tui";

pub fn broker_with_attach(attach_id: &str) -> Broker {
    broker_with_publishes(attach_id, &[])
}

pub fn broker_with_publishes(attach_id: &str, publish_topics: &[&str]) -> Broker {
    let aid = AttachId::new(attach_id).expect("known attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(
        aid,
        FrontendAcl {
            subscribe_patterns: BTreeSet::new(),
            auto_subscribes: BTreeSet::new(),
            publish_topics: publish_topics.iter().map(|s| s.to_string()).collect(),
        },
    );
    Broker::new(BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    })
    .expect("acl is well-formed")
}

pub fn broker_with_attach_and_observer(attach_id: &str, observer: &CanonicalId) -> Broker {
    let aid = AttachId::new(attach_id).expect("known attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(
        aid,
        FrontendAcl {
            subscribe_patterns: BTreeSet::new(),
            auto_subscribes: BTreeSet::new(),
            publish_topics: BTreeSet::new(),
        },
    );
    let mut plugins = BTreeMap::new();
    plugins.insert(
        observer.clone(),
        PluginAcl {
            topic_id: "obs".to_string(),
            publish_topics: Vec::new(),
            subscribe_patterns: vec!["core.lifecycle.**".to_string()],
            auto_subscribes: Vec::new(),
            provider_id: None,
        },
    );
    Broker::new(BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends,
    })
    .expect("acl is well-formed")
}

/// Materialises a 0o755 file under a tempdir and returns it.
pub fn executable_entry() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("frontend-bin");
    std::fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("write entry");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod entry");
    (dir, path)
}

/// Plan whose Phase A would succeed — every test mutates one
/// field to provoke the targeted error.
pub fn baseline_plan(attach_id: &str, entry_absolute: PathBuf) -> CompiledFrontend {
    CompiledFrontend {
        attach_id: attach_id.to_string(),
        entry_absolute,
        argv: Vec::<OsString>::new(),
        env: EnvPlan::default(),
    }
}

pub fn paths() -> FrontendPaths {
    FrontendPaths {
        project_root: PathBuf::from("/tmp/proj-c18"),
    }
}

#[cfg(feature = "test-fixture")]
pub fn fixture_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"))
}

#[cfg(feature = "test-fixture")]
pub fn fixture_plan(attach_id: &str, mode: &str, extra_env: &[(&str, &str)]) -> CompiledFrontend {
    let mut set: BTreeMap<String, String> = BTreeMap::new();
    set.insert("RFL_FIXTURE_MODE".into(), mode.into());
    for (k, v) in extra_env {
        set.insert((*k).to_string(), (*v).to_string());
    }
    CompiledFrontend {
        attach_id: attach_id.to_string(),
        entry_absolute: fixture_bin_path(),
        argv: Vec::<OsString>::new(),
        env: EnvPlan {
            pass: vec!["PATH".to_string()],
            set,
        },
    }
}

#[cfg(feature = "test-fixture")]
pub fn live_paths(tempdir: &TempDir) -> FrontendPaths {
    FrontendPaths {
        project_root: tempdir.path().to_path_buf(),
    }
}
