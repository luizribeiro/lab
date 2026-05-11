//! c31 §OP2 / pi-4 M-2 — `ToolSchemaCatalog::build` returns
//! `ToolMissingOpenRpcMethod` when a `provides.tools` entry has no
//! matching `methods[i].name`.

mod common;

use rafaello_core::supervisor::{ToolCatalogError, ToolSchemaCatalog};

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled, write_openrpc};

#[test]
fn errors_when_openrpc_method_missing_for_tool() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = common::canonical("local/test:mailcat@0.1.0");
    write_openrpc(
        tmp.path(),
        r#"{
          "openrpc": "1.2.6",
          "info": { "title": "mailcat", "version": "0.0.0" },
          "methods": [
            { "name": "send-mail-typo", "params": [] }
          ]
        }"#,
    );

    let acl = make_acl(&canonical, &["send-mail"], None);
    let compiled = single_compiled(&canonical, None);
    let pkgs = package_dirs(&canonical, tmp.path());

    let err = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect_err("expected error");
    match err {
        ToolCatalogError::ToolMissingOpenRpcMethod { canonical: c, tool } => {
            assert_eq!(c, canonical);
            assert_eq!(tool, "send-mail");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
