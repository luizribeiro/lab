//! Materialise a fully-valid `rafaello.lock` with installed
//! mockprovider + readfile plugins, parameterised by which entry
//! binary should be executable. Used by c25's spawn-failure
//! negatives.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use chrono::{DateTime, Utc};
use rafaello_core::digest;
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, LoadPolicy, Lock, LockFlags, PluginEntry, SessionTable,
};
use rafaello_core::manifest::{Manifest, SafePath};
use rafaello_core::topic_id;

const MOCKPROVIDER_CANONICAL: &str = "local/test:mockprov@0.1.0";
const READFILE_CANONICAL: &str = "local/test:readfile@0.1.0";

pub struct InstallOptions {
    pub provider_executable: bool,
    pub tool_executable: bool,
    /// When true, install the actual `rfl-mockprovider` / `rfl-readfile`
    /// binaries so eager spawn produces a live child. When false,
    /// install a `#!/bin/sh` stub (faster, but lockin's syd enforcer
    /// blocks the shebang exec — only useful for failure paths that
    /// short-circuit before the child runs).
    pub real_binaries: bool,
}

pub fn install_demo_layout(project_root: &Path, opts: InstallOptions) {
    let provider_bin: Option<std::path::PathBuf> = if opts.real_binaries {
        Some(super::workspace_bin_path::workspace_bin("rfl-mockprovider"))
    } else {
        None
    };
    let tool_bin: Option<std::path::PathBuf> = if opts.real_binaries {
        Some(super::workspace_bin_path::workspace_bin("rfl-readfile"))
    } else {
        None
    };
    let mp = install_plugin(
        project_root,
        fixture_dir("rafaello-mockprovider"),
        MOCKPROVIDER_CANONICAL,
        "bin/rfl-mockprovider",
        opts.provider_executable,
        provider_bin.as_deref(),
        true,
        Some("mock"),
        &[],
    );
    let rf = install_plugin(
        project_root,
        fixture_dir("rafaello-readfile"),
        READFILE_CANONICAL,
        "bin/rfl-readfile",
        opts.tool_executable,
        tool_bin.as_deref(),
        false,
        None,
        &["read-file"],
    );

    let mut plugins = std::collections::BTreeMap::new();
    plugins.insert(mp.0, mp.1);
    plugins.insert(rf.0, rf.1);

    let lock = Lock {
        plugins,
        session: SessionTable {
            provider_active: Some(MOCKPROVIDER_CANONICAL.to_string()),
            tool_owner: std::collections::BTreeMap::new(),
        },
    };
    let lock_toml = lock.to_toml();
    fs::write(project_root.join("rafaello.lock"), lock_toml).expect("write rafaello.lock");
}

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join(name)
}

#[allow(clippy::too_many_arguments)]
fn install_plugin(
    project_root: &Path,
    fixture: std::path::PathBuf,
    canonical_str: &str,
    entry_rel: &str,
    executable: bool,
    real_binary: Option<&Path>,
    provider: bool,
    provider_id: Option<&str>,
    tools: &[&str],
) -> (CanonicalId, PluginEntry) {
    let canonical = CanonicalId::parse(canonical_str).expect("canonical");
    let topic = topic_id::derive(canonical_str);
    let install_dir = project_root.join(".rafaello").join("plugins").join(&topic);
    fs::create_dir_all(&install_dir).expect("install dir");

    let manifest_raw =
        fs::read_to_string(fixture.join("rafaello.toml")).expect("read fixture manifest");
    fs::write(install_dir.join("rafaello.toml"), &manifest_raw).expect("write manifest");
    let openrpc = fs::read(fixture.join("openrpc.json")).expect("read fixture openrpc");
    fs::write(install_dir.join("openrpc.json"), openrpc).expect("write openrpc");

    let entry_abs = install_dir.join(entry_rel);
    fs::create_dir_all(entry_abs.parent().unwrap()).expect("entry parent");
    match real_binary {
        Some(src) => {
            fs::copy(src, &entry_abs).expect("copy real plugin binary");
        }
        None => {
            fs::write(&entry_abs, b"#!/bin/sh\nexit 0\n").expect("write entry stub");
        }
    }
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&entry_abs, fs::Permissions::from_mode(mode)).expect("chmod entry");

    let manifest = Manifest::parse(&manifest_raw).expect("parse manifest");
    let manifest_digest = digest::manifest_digest(&manifest.canonical_bytes());
    let content_digest = digest::content_digest(&install_dir).expect("content digest");

    let granted_at: DateTime<Utc> = "2026-05-10T00:00:00Z".parse().unwrap();
    let entry = PluginEntry {
        entry: SafePath::parse(entry_rel).unwrap(),
        digest: content_digest,
        manifest_digest,
        granted_at,
        grant: Grant::default(),
        bindings: Bindings {
            provider,
            provider_id: provider_id.map(str::to_string),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            renderer_kinds: Vec::new(),
            tool_meta: std::collections::BTreeMap::new(),
            load: LoadPolicy::default(),
        },
        flags: LockFlags::default(),
    };
    (canonical, entry)
}
