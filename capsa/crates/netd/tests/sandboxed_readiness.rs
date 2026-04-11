//! Spawns capsa-netd inside a real sandbox policy that mirrors what
//! production does in capsa-core's `start::imp`.
//! Catches the class of "policy is missing a path netd's startup
//! needs" regressions: a missing binary path, the linux runtime
//! probe paths (tokio reads `/proc/stat`, `/sys/...cpu/online` to
//! size its thread pool), or new files that get added to the
//! `capsa_net::runtime_read_paths()` list later but never get
//! exercised under the real wrapper.
//!
//! Limitation: this test does **not** catch the silent DNS fallback.
//! `DnsProxy::new` swallows the resolv.conf open error and falls
//! back to 8.8.8.8, so netd starts cleanly even if the policy is
//! missing `/etc/resolv.conf` — the symptom only shows up when a
//! guest issues a DNS query and the wrong upstream answers it.
//! Detecting that requires either a tracing subscriber on
//! capsa-netd or active DNS probing through the host_fd; both are
//! deferred until they have a clear caller.

use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use capsa_test_support::{sandbox_builder, ChildGuard};

const PROBE_MAC: [u8; 6] = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
const READINESS_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn netd_signals_readiness_under_real_sandbox() {
    let netd_bin = PathBuf::from(env!("CARGO_BIN_EXE_capsa-netd"));

    let (ready_reader, ready_writer) = std::io::pipe().expect("create ready pipe");
    let ready_writer_owned: OwnedFd = ready_writer.into();

    let (host_side, _peer_side) = UnixDatagram::pair().expect("create UnixDatagram pair");
    let host_owned: OwnedFd = host_side.into();

    // Build the policy from `capsa_net::runtime_read_paths()`, which
    // is the same source of truth `capsa-core::start::imp` reads
    // from. If that list ever drops a path netd actually opens, this
    // test will fail before production silently falls back.
    let mut builder = sandbox_builder()
        .allow_network(true)
        .read_only_path(netd_bin.clone());
    for runtime_path in capsa_net::runtime_read_paths() {
        builder = builder.read_only_path(PathBuf::from(*runtime_path));
    }
    let host_fd = builder.inherit_fd(host_owned);
    let ready_fd = builder.inherit_fd(ready_writer_owned);

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

    let mut cmd = builder
        .command(&netd_bin)
        .expect("build sandboxed netd command");
    cmd.arg("--launch-spec-json")
        .arg(&spec)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    let sandbox_child = cmd.spawn().expect("spawn sandboxed netd");
    let (child, _sandbox) = sandbox_child.into_parts();
    let _child = ChildGuard::new(child);

    let ready_byte = read_byte_with_timeout(ready_reader, READINESS_TIMEOUT);
    assert_eq!(
        ready_byte, b'R',
        "sandboxed capsa-netd must emit 'R' readiness, got 0x{ready_byte:02x}"
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
        .expect("sandboxed capsa-netd did not signal readiness within timeout")
        .expect("read from readiness pipe")
}
