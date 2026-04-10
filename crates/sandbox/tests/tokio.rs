//! Async factory parity: the tokio build path enforces the same
//! contracts as the sync `SandboxBuilder::build`.
//!
//! Run with `cargo test -p capsa-sandbox --features tokio --test tokio`.

mod common;

use std::net::TcpListener;

use common::{probe_binary, TestDir};

use capsa_sandbox::{Sandbox, SandboxBuilder};

#[tokio::test]
async fn read_allowlist_enforced() {
    let temp = TestDir::new("tokio-read");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");

    std::fs::write(&allowed, b"allowed").expect("write allowed fixture");
    std::fs::write(&sibling, b"sibling").expect("write sibling fixture");

    assert!(
        run_probe(
            Sandbox::builder().read_only_path(allowed.clone()),
            &["can-read", &allowed.display().to_string()]
        )
        .await
    );
    assert!(
        !run_probe(
            Sandbox::builder().read_only_path(allowed.clone()),
            &["can-read", &sibling.display().to_string()]
        )
        .await
    );
}

#[tokio::test]
async fn private_tmpdir_is_writable() {
    assert!(run_probe(Sandbox::builder(), &["can-write-temp"]).await);
}

#[tokio::test]
async fn write_scoped_to_explicit_rw_paths() {
    let temp = TestDir::new("tokio-write");
    let allowed = temp.join("ok.txt");
    let denied = temp.join("nope.txt");
    std::fs::write(&allowed, b"seed").expect("seed allowed");
    std::fs::write(&denied, b"seed").expect("seed denied");

    assert!(
        run_probe(
            Sandbox::builder().read_write_path(allowed.clone()),
            &["can-write", &allowed.display().to_string()]
        )
        .await
    );
    assert!(
        !run_probe(
            Sandbox::builder().read_write_path(allowed),
            &["can-write", &denied.display().to_string()]
        )
        .await
    );
}

#[tokio::test]
async fn network_blocked_when_disabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().expect("addr").port();

    assert!(
        !run_probe(
            Sandbox::builder().allow_network(false),
            &["can-connect", "127.0.0.1", &port.to_string()]
        )
        .await
    );
}

#[tokio::test]
async fn network_allowed_when_enabled() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().expect("addr").port();

    let accept = std::thread::spawn(move || {
        let _ = listener.accept();
    });

    assert!(
        run_probe(
            Sandbox::builder().allow_network(true),
            &["can-connect", "127.0.0.1", &port.to_string()]
        )
        .await
    );

    let _ = accept.join();
}

async fn run_probe(builder: SandboxBuilder, args: &[&str]) -> bool {
    let probe = probe_binary();
    let (mut command, _sandbox) = capsa_sandbox::tokio::build(builder, &probe)
        .unwrap_or_else(|e| panic!("failed to build sandbox for probe {}: {e}", probe.display()));

    let status = command.args(args).status().await.unwrap_or_else(|e| {
        panic!(
            "failed to run sandboxed probe {} with args {:?}: {e}",
            probe.display(),
            args
        )
    });

    status.success()
}
