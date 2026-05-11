//! Sink-default inference per scope §Si1.
//!
//! When a manifest omits `[provides.tool.<n>] sinks = [...]`, the
//! installer snapshots a default sink list inferred from the tool's
//! **effective grant** (per `decisions.md` row 17 / pi review-3
//! finding 3: `default ∪ <tool-name>`). When the manifest declares
//! `sinks` explicitly, that list wins verbatim (including the
//! explicit-empty case).

use crate::lock::{Grant, GrantBundle, GrantFilesystem, GrantNetwork};
use crate::manifest::capabilities::NetworkMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkClass {
    Network,
    VcsPush,
    Mail,
    WorkspaceWrite,
    Other(String),
}

impl SinkClass {
    pub fn parse(s: &str) -> Self {
        match s {
            "network" => SinkClass::Network,
            "vcs_push" => SinkClass::VcsPush,
            "mail" => SinkClass::Mail,
            "workspace_write" => SinkClass::WorkspaceWrite,
            other => SinkClass::Other(other.to_owned()),
        }
    }
}

/// Per scope §Si1: when `declared` is `Some(_)`, return its contents
/// verbatim (no inference). Otherwise infer defaults from the
/// effective per-tool grant bundle.
pub fn infer_defaults(effective: &GrantBundle, declared: &Option<Vec<String>>) -> Vec<String> {
    if let Some(list) = declared {
        return list.clone();
    }

    let mut sinks: Vec<String> = Vec::new();

    if let Some(net) = &effective.network {
        if net.mode != NetworkMode::Deny {
            sinks.push("network".to_owned());
        }
    }

    if let Some(fs) = &effective.filesystem {
        if !fs.write_dirs.is_empty() {
            sinks.push("workspace_write".to_owned());
        }
    }

    sinks.sort();
    sinks
}

/// Effective grant bundle for tool `<n>`: union of the `default`
/// bundle with the `<n>`-named bundle. Per `decisions.md` row 17 +
/// pi review-3 finding 3 — sink inference must see every authority
/// the tool receives at spawn time, including those that arrive only
/// via the tool-named bundle.
pub fn effective_grant(grant: &Grant, tool: &str) -> GrantBundle {
    let mut out = grant.bundles.get("default").cloned().unwrap_or_default();
    if let Some(named) = grant.bundles.get(tool) {
        union_bundle(&mut out, named);
    }
    out
}

fn union_bundle(dst: &mut GrantBundle, src: &GrantBundle) {
    if let Some(src_fs) = &src.filesystem {
        let dst_fs = dst.filesystem.get_or_insert_with(GrantFilesystem::default);
        extend_unique(&mut dst_fs.read_paths, &src_fs.read_paths);
        extend_unique(&mut dst_fs.read_dirs, &src_fs.read_dirs);
        extend_unique(&mut dst_fs.write_paths, &src_fs.write_paths);
        extend_unique(&mut dst_fs.write_dirs, &src_fs.write_dirs);
        extend_unique(&mut dst_fs.exec_paths, &src_fs.exec_paths);
        extend_unique(&mut dst_fs.exec_dirs, &src_fs.exec_dirs);
    }

    if let Some(src_net) = &src.network {
        let dst_net = dst.network.get_or_insert_with(GrantNetwork::default);
        if network_rank(src_net.mode) > network_rank(dst_net.mode) {
            dst_net.mode = src_net.mode;
        }
        extend_unique(&mut dst_net.allow_hosts, &src_net.allow_hosts);
    }
}

fn network_rank(mode: NetworkMode) -> u8 {
    match mode {
        NetworkMode::Deny => 0,
        NetworkMode::Proxy => 1,
        NetworkMode::AllowAll => 2,
    }
}

fn extend_unique(dst: &mut Vec<String>, src: &[String]) {
    for item in src {
        if !dst.iter().any(|x| x == item) {
            dst.push(item.clone());
        }
    }
}
