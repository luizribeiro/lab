//! Tests for the `Sandbox` construction and lifetime contract. Path/network
//! contract tests go through `common::run_probe` and live in the other test
//! files in this directory.

#[cfg(target_os = "linux")]
#[test]
fn sandbox_new_rejects_allow_network_on_linux() {
    let spec = capsa_sandbox::SandboxSpec::new().allow_network(true);
    let err = capsa_sandbox::Sandbox::new(spec)
        .err()
        .expect("Sandbox::new must reject allow_network=true on Linux");
    let msg = err.to_string();
    assert!(
        msg.contains("network"),
        "error should mention the network conflict, got: {msg}"
    );
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
