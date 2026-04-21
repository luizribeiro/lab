//! Network isolation contract: connections are blocked in `Deny`
//! mode (the default) and permitted in `AllowAll`.

mod common;

use std::net::TcpListener;

use common::run_probe;

#[test]
fn connections_blocked_when_network_denied() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
    let port = listener.local_addr().expect("local addr").port();

    assert!(!run_probe(
        common::sandbox_builder().network_deny(),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));
}

#[test]
fn connections_allowed_in_allow_all_mode() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
    let port = listener.local_addr().expect("local addr").port();

    let accept_thread = std::thread::spawn(move || {
        let _ = listener.accept();
    });

    assert!(run_probe(
        common::sandbox_builder().network_allow_all(),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));

    let _ = accept_thread.join();
}
