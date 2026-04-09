mod common;

use std::net::TcpListener;

use common::{run_probe, TestDir};

#[test]
fn network_is_blocked_when_disabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind local listener");
    let port = listener
        .local_addr()
        .expect("failed to read local addr")
        .port();

    let spec = capsa_sandbox::SandboxSpec::new().allow_network(false);
    assert!(!run_probe(
        &spec,
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));
}

// Linux's `syd` backend cannot currently sandbox processes that also need
// network access; `Sandbox::new` hard-errors on that combination and callers
// must fall back to unsandboxed spawn. On macOS the seatbelt backend supports
// `allow_network` natively, so this test only runs there.
#[cfg(target_os = "macos")]
#[test]
fn network_is_allowed_when_enabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind local listener");
    let port = listener
        .local_addr()
        .expect("failed to read local addr")
        .port();

    let accept_thread = std::thread::spawn(move || {
        let _ = listener.accept();
    });

    let spec = capsa_sandbox::SandboxSpec::new().allow_network(true);
    assert!(run_probe(
        &spec,
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));

    let _ = accept_thread.join();
}

#[test]
fn writes_do_not_target_global_tmp_without_explicit_allowlist() {
    let temp = TestDir::new("tmp-contract");
    let host_tmp_file = temp.join("host-tmp-target.txt");
    std::fs::write(&host_tmp_file, b"seed").expect("failed to seed host tmp file");

    let spec = capsa_sandbox::SandboxSpec::new();

    assert!(!run_probe(
        &spec,
        &["can-write", &host_tmp_file.display().to_string()]
    ));
}

#[test]
fn writes_can_target_private_tmpdir() {
    let spec = capsa_sandbox::SandboxSpec::new();
    assert!(run_probe(&spec, &["can-write-temp"]));
}
