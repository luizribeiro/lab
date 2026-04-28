//! Builder lifecycle: can we build a sandbox, and does the private
//! tmp directory outlive the child?

use std::path::Path;

mod common;

#[test]
fn command_accepts_network_allow_all() {
    common::sandbox_builder()
        .network_allow_all()
        .command(Path::new("/usr/bin/env"))
        .expect("builder.command must accept network_allow_all()");
}

#[test]
fn private_tmp_lives_until_drop() {
    let mut child = common::sandbox_builder()
        .command(Path::new("/usr/bin/true"))
        .expect("build sandbox")
        .spawn()
        .expect("spawn /usr/bin/true");
    child.wait().expect("wait for child");
    let (_child, sandbox) = child.into_parts();

    let tmp = sandbox.private_tmp().to_path_buf();
    assert!(tmp.is_dir(), "private tmp should exist while sandbox held");

    drop(sandbox);
    assert!(
        !tmp.exists(),
        "private tmp should be removed when sandbox is dropped"
    );
}
