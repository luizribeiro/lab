//! c31 §OP2 / pi-1 B-9 — `ToolSchemaCatalog::build` against c30's
//! `rafaello-mailcat-method-typo` fixture (openrpc declares
//! `send-male`, manifest declares `send-mail`) returns
//! `ToolCatalogError::ToolMissingOpenRpcMethod`.

mod common;

use std::path::PathBuf;

use rafaello_core::supervisor::{ToolCatalogError, ToolSchemaCatalog};

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled};

fn typo_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("rafaello-mailcat-method-typo")
}

#[test]
fn mailcat_typo_catalog_errors_with_tool_missing_openrpc_method() {
    let canonical = common::canonical("local/test:mailcat@0.1.0");
    let acl = make_acl(&canonical, &["send-mail"], None);
    let compiled = single_compiled(&canonical, None);
    let pkgs = package_dirs(&canonical, &typo_fixture());

    let err = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect_err("expected error");
    match err {
        ToolCatalogError::ToolMissingOpenRpcMethod { canonical: c, tool } => {
            assert_eq!(c, canonical);
            assert_eq!(tool, "send-mail");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
