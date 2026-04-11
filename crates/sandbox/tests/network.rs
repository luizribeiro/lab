//! Network isolation contract: connections are blocked when
//! `allow_network(false)` and permitted when `allow_network(true)`.

mod common;

use std::net::TcpListener;

use common::run_probe;

#[test]
fn connections_blocked_when_network_disabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
    let port = listener.local_addr().expect("local addr").port();

    assert!(!run_probe(
        common::sandbox_builder().allow_network(false),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));
}

#[test]
fn connections_allowed_when_network_enabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
    let port = listener.local_addr().expect("local addr").port();

    let accept_thread = std::thread::spawn(move || {
        let _ = listener.accept();
    });

    assert!(run_probe(
        common::sandbox_builder().allow_network(true),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));

    let _ = accept_thread.join();
}
