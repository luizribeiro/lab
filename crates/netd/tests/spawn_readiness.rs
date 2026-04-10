//! End-to-end spawn test: verifies the `capsa-netd` binary parses a
//! launch spec via `--launch-spec-json`, inherits the specified fds,
//! and signals readiness before serving interface traffic.
//!
//! Unit tests in `src/runtime.rs` drive `run()` directly and cannot
//! exercise the `pre_exec` → `exec` fd-inheritance path; this test
//! fills that gap by spawning the real binary as a subprocess.

use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use capsa_sandbox::configure_inherited_fds;
use capsa_test_support::ChildGuard;

const PROBE_MAC: [u8; 6] = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const LIVENESS_PROBE_DELAY: Duration = Duration::from_millis(200);

#[test]
fn netd_binary_signals_readiness_after_spawn() {
    let (ready_reader, ready_writer) = std::io::pipe().expect("create ready pipe");
    let ready_writer_owned: OwnedFd = ready_writer.into();
    let ready_fd = ready_writer_owned.as_raw_fd();

    let (host_side, peer_side) = UnixDatagram::pair().expect("create UnixDatagram pair");
    let host_owned: OwnedFd = host_side.into();
    let host_fd = host_owned.as_raw_fd();

    let spec = serde_json::json!({
        "ready_fd": ready_fd,
        "interfaces": [{
            "host_fd": host_fd,
            "mac": PROBE_MAC,
            "policy": null,
        }],
        "port_forwards": [],
    })
    .to_string();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_capsa-netd"));
    cmd.arg("--launch-spec-json")
        .arg(&spec)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    // configure_inherited_fds consumes the OwnedFds; the raw fd numbers
    // baked into the spec JSON above must already be captured.
    configure_inherited_fds(&mut cmd, vec![ready_writer_owned, host_owned], false)
        .expect("configure_inherited_fds");

    let mut child = ChildGuard::new(cmd.spawn().expect("spawn capsa-netd"));

    let ready_byte = read_byte_with_timeout(ready_reader, READINESS_TIMEOUT);
    assert_eq!(
        ready_byte, b'R',
        "capsa-netd must emit the 'R' readiness marker, got 0x{ready_byte:02x}"
    );

    peer_side
        .send(&sample_ethernet_frame())
        .expect("send probe frame on peer socketpair");

    // Give the daemon a moment to process the frame; if the interface
    // fd handoff is actually broken, wait_fail_fast would observe a
    // task error within this window.
    thread::sleep(LIVENESS_PROBE_DELAY);
    assert!(
        child.child.try_wait().expect("try_wait").is_none(),
        "capsa-netd should still be running after readiness + interface traffic"
    );
}

fn read_byte_with_timeout<R>(mut reader: R, timeout: Duration) -> u8
where
    R: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0u8; 1];
        let _ = tx.send(reader.read_exact(&mut buf).map(|_| buf[0]));
    });

    rx.recv_timeout(timeout)
        .expect("capsa-netd did not signal readiness within timeout")
        .expect("read from readiness pipe")
}

fn sample_ethernet_frame() -> Vec<u8> {
    const BROADCAST_MAC: [u8; 6] = [0xff; 6];
    const ETHERTYPE_IPV4: [u8; 2] = [0x08, 0x00];

    let mut frame = vec![0u8; 64];
    frame[0..6].copy_from_slice(&BROADCAST_MAC);
    frame[6..12].copy_from_slice(&PROBE_MAC);
    frame[12..14].copy_from_slice(&ETHERTYPE_IPV4);
    frame
}
