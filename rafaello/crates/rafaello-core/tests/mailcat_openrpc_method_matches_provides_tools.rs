//! c31 §OP2 / pi-1 B-9 — `ToolSchemaCatalog::build` succeeds on
//! c30's `rafaello-mailcat-good` fixture and the synthesised
//! `parameters_schema` covers `to/subject/body`.

mod common;

use std::path::PathBuf;

use rafaello_core::supervisor::ToolSchemaCatalog;
use serde_json::Value;

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled};

fn mailcat_good_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("rafaello-mailcat-good")
}

#[test]
fn mailcat_good_catalog_builds_with_send_mail_params() {
    let canonical = common::canonical("local/test:mailcat@0.1.0");
    let acl = make_acl(&canonical, &["send-mail"], None);
    let compiled = single_compiled(&canonical, None);
    let pkgs = package_dirs(&canonical, &mailcat_good_fixture());

    let cat = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect("build");
    assert_eq!(cat.list().len(), 1);
    let schema = &cat.list()[0].parameters_schema;
    let props = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties");
    assert!(props.contains_key("to"));
    assert!(props.contains_key("subject"));
    assert!(props.contains_key("body"));
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .expect("required");
    assert_eq!(required, vec!["to"]);
}
