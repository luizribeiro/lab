//! Builder lifecycle: can we build a sandbox, and does the private
//! tmp directory outlive the child?

use std::path::Path;

use capsa_sandbox::Sandbox;

#[test]
fn build_accepts_allow_network() {
    Sandbox::builder()
        .allow_network(true)
        .build(Path::new("/usr/bin/env"))
        .expect("builder.build must accept allow_network=true");
}

#[test]
fn private_tmp_lives_until_drop() {
    let (_cmd, sandbox) = Sandbox::builder()
        .build(Path::new("/usr/bin/env"))
        .expect("build sandbox");
    let tmp = sandbox.private_tmp().to_path_buf();
    assert!(tmp.is_dir(), "private tmp should exist while sandbox held");

    drop(sandbox);
    assert!(
        !tmp.exists(),
        "private tmp should be removed when sandbox is dropped"
    );
}
