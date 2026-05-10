#![allow(dead_code)]
//! Shared builders for c18+ frontend Phase A negative tests.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::EnvPlan;
use rafaello_core::frontend::{CompiledFrontend, FrontendPaths};
use tempfile::TempDir;

pub const KNOWN_ATTACH_ID: &str = "tui";

pub fn broker_with_attach(attach_id: &str) -> Broker {
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
    Broker::new(BrokerAcl {
        plugins: BTreeMap::new(),
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
