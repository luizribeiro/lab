//! Integration tests for the tokio flavor of the `Sandbox::command` factory.
//!
//! Run with `cargo test -p capsa-sandbox --features tokio --test sandbox_tokio`.

mod common;

use common::{probe_binary, TestDir};

#[tokio::test]
async fn tokio_command_factory_enforces_read_allowlist() {
    let temp = TestDir::new("tokio-read");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");

    std::fs::write(&allowed, b"allowed").expect("write allowed fixture");
    std::fs::write(&sibling, b"sibling").expect("write sibling fixture");

    let mut spec = capsa_sandbox::SandboxSpec::new();
    spec.read_only_paths.push(allowed.clone());

    assert!(run_probe(&spec, &["can-read", &allowed.display().to_string()]).await);
    assert!(!run_probe(&spec, &["can-read", &sibling.display().to_string()]).await);
}

#[tokio::test]
async fn tokio_command_factory_grants_write_to_private_tmp() {
    let spec = capsa_sandbox::SandboxSpec::new();
    assert!(run_probe(&spec, &["can-write-temp"]).await);
}

async fn run_probe(spec: &capsa_sandbox::SandboxSpec, args: &[&str]) -> bool {
    let probe = probe_binary();
    let sandbox = capsa_sandbox::Sandbox::new(spec.clone())
        .unwrap_or_else(|e| panic!("failed to build sandbox for probe {}: {e}", probe.display()));

    let status = capsa_sandbox::tokio::command(&sandbox, &probe)
        .args(args)
        .status()
        .await
        .unwrap_or_else(|e| {
            panic!(
                "failed to run sandboxed probe {} with args {:?}: {e}",
                probe.display(),
                args
            )
        });

    status.success()
}
