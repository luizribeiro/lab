//! c31 §OP2 — `ToolSchemaCatalog::build` synthesises a JSON-Schema
//! `parameters_schema` from an `openrpc.json` method's `params`.

mod common;

use rafaello_core::supervisor::ToolSchemaCatalog;
use serde_json::json;

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled, write_openrpc};

#[test]
fn synthesises_parameters_schema_object_from_openrpc_params() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = common::canonical("local/test:tool@0.1.0");
    write_openrpc(
        tmp.path(),
        r#"{
          "openrpc": "1.2.6",
          "info": { "title": "t", "version": "0.0.0" },
          "methods": [
            {
              "name": "do-thing",
              "params": [
                { "name": "path", "required": true,
                  "schema": { "type": "string" } },
                { "name": "limit", "required": false,
                  "schema": { "type": "integer" } }
              ]
            }
          ]
        }"#,
    );

    let acl = make_acl(&canonical, &["do-thing"], None);
    let compiled = single_compiled(&canonical, None);
    let pkgs = package_dirs(&canonical, tmp.path());

    let cat = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect("build");
    assert_eq!(cat.list().len(), 1);
    let s = &cat.list()[0];
    assert_eq!(s.name, "do-thing");
    assert_eq!(
        s.parameters_schema,
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "limit": { "type": "integer" }
            },
            "required": ["path"]
        })
    );
}
