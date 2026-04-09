mod common;

use std::net::TcpListener;

use common::{run_probe, TestDir};

use capsa_sandbox::Sandbox;

#[test]
fn network_is_blocked_when_disabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind local listener");
    let port = listener
        .local_addr()
        .expect("failed to read local addr")
        .port();

    assert!(!run_probe(
        Sandbox::builder().allow_network(false),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));
}

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

    assert!(run_probe(
        Sandbox::builder().allow_network(true),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));

    let _ = accept_thread.join();
}

#[test]
fn writes_do_not_target_global_tmp_without_explicit_allowlist() {
    let temp = TestDir::new("tmp-contract");
    let host_tmp_file = temp.join("host-tmp-target.txt");
    std::fs::write(&host_tmp_file, b"seed").expect("failed to seed host tmp file");

    assert!(!run_probe(
        Sandbox::builder(),
        &["can-write", &host_tmp_file.display().to_string()]
    ));
}

#[test]
fn writes_can_target_private_tmpdir() {
    assert!(run_probe(Sandbox::builder(), &["can-write-temp"]));
}
