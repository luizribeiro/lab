//! Cross-platform deny-by-default behavioral tests.
//!
//! Each test asserts a deny invariant from lockin's contract on both
//! Linux (syd + Landlock) and macOS (Seatbelt + system.sb baseline +
//! M2a hardening denies). No `#[cfg(target_os)]` gating: the same
//! probe action and the same assertion run on both platforms. Where a
//! probe needs a target that doesn't exist on every test runner (e.g.
//! a syslog socket), the test does a runtime `Path::exists()` skip.

mod common;

use std::net::{TcpListener, UdpSocket};
use std::os::unix::net::UnixListener;
use std::path::Path;

use common::{run_probe, TestDir};

// ── filesystem deny outside allowlist ────────────────────────

#[test]
fn read_outside_tempdir_is_denied_with_no_allowlist() {
    let producer = TestDir::new("read-outside-producer");
    let secret = producer.join("secret.txt");
    std::fs::write(&secret, b"top secret").expect("seed secret");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-read", &secret.display().to_string()]
    ));
}

#[test]
fn write_outside_tempdir_is_denied_with_no_allowlist() {
    let producer = TestDir::new("write-outside-producer");
    let target = producer.join("target.txt");
    std::fs::write(&target, b"orig").expect("seed target");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-write", &target.display().to_string()]
    ));

    let after = std::fs::read(&target).expect("read after attempt");
    assert_eq!(after, b"orig", "denied write must not have modified file");
}

// ── exec deny outside allowlist ──────────────────────────────

#[test]
fn exec_outside_allowlist_is_denied() {
    // /usr/bin/true exits 0, so without a sandbox-enforced exec deny
    // the probe would succeed; with deny, spawn fails and probe
    // returns nonzero. /bin/true is preferred on Linux distros that
    // don't ship /usr/bin/true.
    let candidates = ["/usr/bin/true", "/bin/true"];
    let outside = candidates
        .iter()
        .copied()
        .find(|p| Path::new(p).exists())
        .expect("expected /usr/bin/true or /bin/true on host");

    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-exec", outside]
    ));
}

// ── /cores write deny (M2a unconditional deny on macOS; outside
//    allowlist on Linux — same outcome from the caller's view) ──

#[test]
fn write_to_cores_is_denied() {
    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-write", "/cores/lockin-test-deny"]
    ));
}

// ── network deny: TCP IPv4 (already covered in network.rs;
//    re-asserted here for symmetry with IPv6) ──

#[test]
fn tcp_connect_ipv4_loopback_is_denied() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind v4 listener");
    let port = listener.local_addr().expect("local addr").port();

    assert!(!run_probe(
        common::sandbox_builder().network_deny(),
        &["can-connect", "127.0.0.1", &port.to_string()]
    ));
}

#[test]
fn tcp_connect_ipv6_loopback_is_denied() {
    // Some CI runners disable IPv6; if we can't even bind a v6
    // listener on the host, the test runner can't make the
    // assertion meaningful, so skip.
    let Ok(listener) = TcpListener::bind("[::1]:0") else {
        eprintln!("skip: host has no IPv6 loopback");
        return;
    };
    let port = listener.local_addr().expect("local addr").port();

    assert!(!run_probe(
        common::sandbox_builder().network_deny(),
        &["can-connect", "::1", &port.to_string()]
    ));
}

#[test]
fn udp_send_to_loopback_is_denied() {
    // Pre-bind on the host to verify the send target is valid in
    // principle (so a denied probe is denied by sandbox, not by
    // some unrelated kernel state).
    let host_sock = UdpSocket::bind("127.0.0.1:0").expect("bind host udp");
    let port = host_sock.local_addr().expect("local addr").port();

    assert!(!run_probe(
        common::sandbox_builder().network_deny(),
        &["can-udp-send", "127.0.0.1", &port.to_string()]
    ));
}

#[test]
fn unix_stream_connect_to_outside_path_is_denied() {
    let temp = TestDir::new("unix-stream-deny");
    let sock_path = temp.join("listener.sock");
    let _listener = UnixListener::bind(&sock_path).expect("bind unix listener");

    assert!(!run_probe(
        common::sandbox_builder().network_deny(),
        &["can-unix-stream-connect", &sock_path.display().to_string()]
    ));
}

#[test]
fn syslog_unix_socket_is_denied() {
    // macOS: /private/var/run/syslog (DGRAM, world-writable). M2a
    // adds an explicit (deny network-outbound) for this literal on
    // top of the system.sb baseline allow, so the deny here proves
    // M2a's hardening works.
    //
    // Linux: /dev/log if syslogd/journald is wired to it. Many CI
    // containers do not ship one — skip in that case so the test
    // remains portable. We assert sandbox denies, NOT
    // "no listener => ENOENT": we runtime-check Path::exists() so a
    // missing path is a skip, not a false pass.
    let candidates = ["/private/var/run/syslog", "/dev/log"];
    let Some(path) = candidates.iter().copied().find(|p| Path::new(p).exists()) else {
        eprintln!("skip: no syslog DGRAM socket present on host");
        return;
    };

    assert!(!run_probe(
        common::sandbox_builder().network_deny(),
        &["can-unix-dgram-connect", path]
    ));
}
