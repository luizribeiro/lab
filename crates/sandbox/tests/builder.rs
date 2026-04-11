//! Builder lifecycle: can we build a sandbox, and does the private
//! tmp directory outlive the child?

use std::path::Path;

mod common;

#[test]
fn command_accepts_allow_network() {
    common::sandbox_builder()
        .allow_network(true)
        .command(Path::new("/usr/bin/env"))
        .expect("builder.command must accept allow_network=true");
}

#[test]
fn private_tmp_lives_until_drop() {
    let (_, sandbox) = common::sandbox_builder()
        .command(Path::new("/usr/bin/env"))
        .expect("build sandbox")
        .into_parts();
    let tmp = sandbox.private_tmp().to_path_buf();
    assert!(tmp.is_dir(), "private tmp should exist while sandbox held");

    drop(sandbox);
    assert!(
        !tmp.exists(),
        "private tmp should be removed when sandbox is dropped"
    );
}
