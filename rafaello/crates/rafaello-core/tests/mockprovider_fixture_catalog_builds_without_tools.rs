//! c31 / pi-2 B-2 — `ToolSchemaCatalog::build` against
//! `rafaello/fixtures/rafaello-mockprovider/` (which declares
//! `[provides] provider = "mock"` and no `tools = [...]`) returns
//! an empty catalog without error.

mod common;

use std::path::PathBuf;

use rafaello_core::supervisor::ToolSchemaCatalog;

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled};

fn mockprovider_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("rafaello-mockprovider")
}

#[test]
fn mockprovider_catalog_builds_with_no_tools() {
    let canonical = common::canonical("local/test:mockprov@0.1.0");
    let acl = make_acl(&canonical, &[], Some("mock"));
    let compiled = single_compiled(&canonical, Some("mock"));
    let pkgs = package_dirs(&canonical, &mockprovider_fixture());

    let cat = ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect("build");
    assert!(
        cat.list().is_empty(),
        "expected empty catalog, got {:?}",
        cat.list()
    );
}
