//! c31 / pi-2 B-2 — `ToolSchemaCatalog::build` succeeds against
//! `rafaello/fixtures/rafaello-readfile/` after this commit's
//! openrpc backfill. Asserts the synthesised `parameters_schema`
//! for `read-file` carries `path`.

mod common;

use std::path::PathBuf;

use rafaello_core::supervisor::ToolSchemaCatalog;
use serde_json::Value;

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled};

fn readfile_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("rafaello-readfile")
}

#[test]
fn readfile_catalog_builds_with_backfilled_openrpc() {
    let canonical = common::canonical("local/test:readfile@0.1.0");
    let acl = make_acl(&canonical, &["read-file"], None);
    let compiled = single_compiled(&canonical, None);
    let pkgs = package_dirs(&canonical, &readfile_fixture());

    let cat = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect("build");
    assert_eq!(cat.list().len(), 1);
    let s = &cat.list()[0];
    assert_eq!(s.name, "read-file");
    let props = s
        .parameters_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties");
    assert!(props.contains_key("path"));
}
