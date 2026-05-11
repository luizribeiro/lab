//! c31 §OP2 item 7 — the synthesised `ToolSchema.parameters_schema`
//! does NOT carry the gate-side fields `sinks`, `grant_match`, or
//! `always_confirm` (those are tool_meta concerns, not model
//! input). Build against a `CompiledPlugin` whose `tool_meta` sets
//! all three, then assert their absence from the schema JSON.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::ToolMeta;
use rafaello_core::supervisor::ToolSchemaCatalog;

use common::tool_catalog_kit::{make_acl, make_compiled, write_openrpc};

#[test]
fn omits_sinks_grant_match_always_confirm_from_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = common::canonical("local/test:mailcat@0.1.0");
    write_openrpc(
        tmp.path(),
        r#"{
          "openrpc": "1.2.6",
          "info": { "title": "mailcat", "version": "0.0.0" },
          "methods": [
            {
              "name": "send-mail",
              "params": [
                { "name": "to", "required": true,
                  "schema": { "type": "string" } }
              ]
            }
          ]
        }"#,
    );

    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "send-mail".to_string(),
        ToolMeta {
            sinks: vec!["mail".to_string()],
            sinks_inferred: false,
            grant_match: Some(PathBuf::from("schemas/send-mail-grant.json")),
            always_confirm: true,
        },
    );
    let mut compiled = BTreeMap::new();
    compiled.insert(
        canonical.clone(),
        make_compiled(&canonical, None, tool_meta),
    );

    let acl = make_acl(&canonical, &["send-mail"], None);
    let mut pkgs = BTreeMap::new();
    pkgs.insert(canonical.clone(), tmp.path().to_path_buf());

    let cat = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect("build");
    let s = &cat.list()[0];
    let json = serde_json::to_string(&s.parameters_schema).unwrap();
    assert!(
        !json.contains("sinks"),
        "schema must not include sinks: {json}"
    );
    assert!(
        !json.contains("grant_match"),
        "schema must not include grant_match: {json}"
    );
    assert!(
        !json.contains("always_confirm"),
        "schema must not include always_confirm: {json}"
    );
}
