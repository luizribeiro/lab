//! Tests for the `Sandbox` construction and lifetime contract. Path/network
//! contract tests go through `common::run_probe` and live in the other test
//! files in this directory.

#[test]
fn sandbox_new_accepts_allow_network() {
    // Sandbox::new must succeed for network-enabled specs on every
    // supported platform; backends that cannot enforce network isolation
    // (e.g. Linux `syd` on kernels with Landlock network rules) fall back
    // to seccomp-only sandboxing rather than erroring.
    let spec = capsa_sandbox::SandboxSpec::new().allow_network(true);
    capsa_sandbox::Sandbox::new(spec).expect("Sandbox::new must accept allow_network=true");
}

#[test]
fn sandbox_private_tmp_lives_until_drop() {
    let sandbox =
        capsa_sandbox::Sandbox::new(capsa_sandbox::SandboxSpec::new()).expect("build sandbox");
    let tmp = sandbox.private_tmp().to_path_buf();
    assert!(tmp.is_dir(), "private tmp should exist while sandbox held");

    drop(sandbox);
    assert!(
        !tmp.exists(),
        "private tmp should be removed when sandbox is dropped"
    );
}
