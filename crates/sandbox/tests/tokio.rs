//! Async factory parity: the tokio build path enforces the same
//! filesystem contracts as the sync `SandboxBuilder::build`.
//!
//! Run with `cargo test -p capsa-sandbox --features tokio --test tokio`.

mod common;

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
