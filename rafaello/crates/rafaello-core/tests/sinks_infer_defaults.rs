//! c18 — `sinks::infer_defaults` covers the scope §Si1 table.
//!
//! - write-only effective grant → `["workspace_write"]`
//! - network-only effective grant → `["network"]`
//! - both → both
//! - neither → `[]`
//! - declared `Some(_)` (including explicit empty) preserved verbatim.

use rafaello_core::lock::{GrantBundle, GrantFilesystem, GrantNetwork};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::sinks::infer_defaults;

fn write_only() -> GrantBundle {
    GrantBundle {
        filesystem: Some(GrantFilesystem {
            write_dirs: vec!["${project}/build".to_owned()],
            ..GrantFilesystem::default()
        }),
        ..GrantBundle::default()
    }
}

fn network_only(mode: NetworkMode) -> GrantBundle {
    GrantBundle {
        network: Some(GrantNetwork {
            mode,
            allow_hosts: Vec::new(),
        }),
        ..GrantBundle::default()
    }
}

#[test]
fn write_only_grant_infers_workspace_write() {
    assert_eq!(
        infer_defaults(&write_only(), &None),
        vec!["workspace_write".to_owned()]
    );
}

#[test]
fn network_proxy_grant_infers_network() {
    assert_eq!(
        infer_defaults(&network_only(NetworkMode::Proxy), &None),
        vec!["network".to_owned()]
    );
}

#[test]
fn network_allow_all_grant_infers_network() {
    assert_eq!(
        infer_defaults(&network_only(NetworkMode::AllowAll), &None),
        vec!["network".to_owned()]
    );
}

#[test]
fn both_authorities_infers_both_sinks() {
    let bundle = GrantBundle {
        filesystem: Some(GrantFilesystem {
            write_dirs: vec!["${project}/out".to_owned()],
            ..GrantFilesystem::default()
        }),
        network: Some(GrantNetwork {
            mode: NetworkMode::Proxy,
            allow_hosts: Vec::new(),
        }),
        ..GrantBundle::default()
    };
    assert_eq!(
        infer_defaults(&bundle, &None),
        vec!["network".to_owned(), "workspace_write".to_owned()]
    );
}

#[test]
fn neither_authority_infers_empty() {
    let bundle = GrantBundle::default();
    let out: Vec<String> = infer_defaults(&bundle, &None);
    assert!(out.is_empty(), "no authority → no inferred sinks");
}

#[test]
fn deny_network_does_not_infer_network_sink() {
    assert_eq!(
        infer_defaults(&network_only(NetworkMode::Deny), &None),
        Vec::<String>::new()
    );
}

#[test]
fn declared_some_wins_verbatim() {
    let bundle = write_only();
    let declared = Some(vec!["custom_sink".to_owned()]);
    assert_eq!(
        infer_defaults(&bundle, &declared),
        vec!["custom_sink".to_owned()],
        "declared list wins over inference"
    );
}

#[test]
fn declared_explicit_empty_preserved() {
    let bundle = GrantBundle {
        filesystem: Some(GrantFilesystem {
            write_dirs: vec!["${project}/out".to_owned()],
            ..GrantFilesystem::default()
        }),
        network: Some(GrantNetwork {
            mode: NetworkMode::Proxy,
            allow_hosts: Vec::new(),
        }),
        ..GrantBundle::default()
    };
    let declared: Option<Vec<String>> = Some(Vec::new());
    let out = infer_defaults(&bundle, &declared);
    assert!(
        out.is_empty(),
        "explicit empty declaration is preserved verbatim, not inferred"
    );
}
