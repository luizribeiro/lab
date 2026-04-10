//! Spike E: a byte traverses two independently sandboxed children
//! through a shared `UnixDatagram` socketpair.
//!
//! This mirrors the production capsa-netd / capsa-vmm fd chain on a
//! simpler scale: both daemons hold one end of a socketpair and run
//! under separate `SandboxBuilder` policies. Spikes A–D covered each
//! piece in isolation; this test exercises the concurrent-two-sandbox
//! topology end-to-end so Phase 1 orchestration can assume it works.

mod common;

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixDatagram;
use std::process::{ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use capsa_sandbox::Sandbox;

use common::{probe_binary, ChildGuard};

const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const MARKER: &str = "X";

#[test]
fn socketpair_byte_traverses_two_sandboxes() {
    let (writer_end, reader_end) = UnixDatagram::pair().expect("unix datagram pair");
    let writer_owned: OwnedFd = writer_end.into();
    let reader_owned: OwnedFd = reader_end.into();

    let probe = probe_binary();

    let mut writer_builder = Sandbox::builder();
    let writer_raw = writer_builder
        .inherit_fd(writer_owned)
        .expect("inherit writer fd");
    let (mut writer_cmd, _writer_sandbox_guard) =
        writer_builder.build(&probe).expect("build writer sandbox");
    writer_cmd
        .arg("fd-write-byte")
        .arg(writer_raw.to_string())
        .arg(MARKER)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut reader_builder = Sandbox::builder();
    let reader_raw = reader_builder
        .inherit_fd(reader_owned)
        .expect("inherit reader fd");
    let (mut reader_cmd, _reader_sandbox_guard) =
        reader_builder.build(&probe).expect("build reader sandbox");
    reader_cmd
        .arg("fd-read-byte")
        .arg(reader_raw.to_string())
        .arg(MARKER)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Spawn both before waiting on either: we want both sandboxes
    // concurrent to mirror the production netd+vmm topology.
    let mut writer_child = ChildGuard::new(writer_cmd.spawn().expect("spawn writer probe"));
    let mut reader_child = ChildGuard::new(reader_cmd.spawn().expect("spawn reader probe"));

    let writer_status = wait_within(&mut writer_child, EXCHANGE_TIMEOUT, "writer");
    assert!(
        writer_status.success(),
        "writer probe failed: status = {writer_status:?}"
    );

    let reader_status = wait_within(&mut reader_child, EXCHANGE_TIMEOUT, "reader");
    assert!(
        reader_status.success(),
        "reader probe failed: status = {reader_status:?}"
    );
}

fn wait_within(child: &mut ChildGuard, timeout: Duration, label: &str) -> ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        match child.child.try_wait() {
            Ok(Some(status)) => return status,
            Ok(None) => {}
            Err(e) => panic!("{label} try_wait failed: {e}"),
        }
        if Instant::now() >= deadline {
            panic!("{label} probe did not exit within {timeout:?}");
        }
        thread::sleep(POLL_INTERVAL);
    }
}
