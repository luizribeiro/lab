//! Tests for `SandboxBuilder::build` and the sandbox lifetime contract.
//! Path/network contract tests go through `common::run_probe` and live
//! in the other test files in this directory.

use std::path::Path;

use capsa_sandbox::Sandbox;

#[test]
fn build_accepts_allow_network() {
    // SandboxBuilder::build must succeed for network-enabled sandboxes on
    // every supported platform; backends that cannot enforce network
    // isolation (e.g. Linux `syd` on kernels with Landlock network
    // rules) fall back to seccomp-only sandboxing rather than erroring.
    Sandbox::builder()
        .allow_network(true)
        .build(Path::new("/bin/true"))
        .expect("builder.build must accept allow_network=true");
}

#[test]
fn sandbox_private_tmp_lives_until_drop() {
    let (_cmd, sandbox) = Sandbox::builder()
        .build(Path::new("/bin/true"))
        .expect("build sandbox");
    let tmp = sandbox.private_tmp().to_path_buf();
    assert!(tmp.is_dir(), "private tmp should exist while sandbox held");

    drop(sandbox);
    assert!(
        !tmp.exists(),
        "private tmp should be removed when sandbox is dropped"
    );
}
