//! c16 §L1a — `probe_fd_closed` mode parses `--probe-fd <N>`,
//! calls `fcntl(N, F_GETFD)`, exits 0 on `EBADF`, non-zero
//! otherwise. Used by m3 lock-fd inheritance tests.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::os::fd::AsRawFd;
use std::time::Duration;

use common::fixture_smoke::spawn_fixture_no_bus;

#[tokio::test(flavor = "multi_thread")]
async fn probe_fd_closed_returns_zero_for_closed_fd() {
    let probe_fd = "999";
    let mut child = spawn_fixture_no_bus("probe_fd_closed", &["--probe-fd", probe_fd], &[]);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("probe wait timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(0), "EBADF must yield exit 0");
}

#[tokio::test(flavor = "multi_thread")]
async fn probe_fd_closed_returns_nonzero_for_open_fd() {
    let dev_null = std::fs::OpenOptions::new()
        .read(true)
        .open("/dev/null")
        .expect("open /dev/null");
    let raw = dev_null.as_raw_fd();

    nix::fcntl::fcntl(
        raw,
        nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::empty()),
    )
    .expect("clear cloexec on probe fd");

    let mut child = spawn_fixture_no_bus("probe_fd_closed", &["--probe-fd", &raw.to_string()], &[]);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("probe wait timed out")
        .expect("child wait");
    assert_ne!(status.code(), Some(0), "open fd must NOT yield exit 0");
    drop(dev_null);
}
