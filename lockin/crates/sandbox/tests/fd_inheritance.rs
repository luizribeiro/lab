//! Inherited file descriptor contract: fds registered via
//! `SandboxBuilder::inherit_fd` survive the sandbox wrapper exec
//! (sandbox-exec on macOS, syd on Linux), and two independently
//! sandboxed children can exchange data through a shared kernel
//! object (UnixDatagram socketpair).
//!
//! Also verifies that non-inherited fds >= 3 are sealed (closed at
//! exec) by default.

mod common;

use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::process::{ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lockin::SandboxChild;

use common::probe_binary;

// ── single-process fd inheritance ────────────────────────────

fn pipe_with_byte(byte: u8) -> OwnedFd {
    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(&[byte]).expect("write marker byte");
    drop(write_end);
    read_end.into()
}

fn run_probe_with_fd_pairs(pairs: Vec<(u8, OwnedFd)>) {
    let mut builder = common::sandbox_builder();
    let mut pair_args: Vec<String> = Vec::with_capacity(pairs.len() * 2);
    for (expected_byte, owned) in pairs {
        let pre_raw = owned.as_raw_fd();
        let raw = builder.inherit_fd(owned);
        assert_eq!(raw, pre_raw);
        pair_args.push(raw.to_string());
        pair_args.push(std::str::from_utf8(&[expected_byte]).unwrap().to_string());
    }

    let probe = probe_binary();
    let mut cmd = builder.command(&probe).expect("build sandbox for probe");
    cmd.arg("fd-read-byte")
        .args(&pair_args)
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd.status().expect("spawn sandboxed probe");
    assert!(
        status.success(),
        "probe failed for inherited fds {pair_args:?}; status = {status:?}"
    );
}

#[test]
fn single_inherited_fd_survives_sandbox_wrapper() {
    run_probe_with_fd_pairs(vec![(b'K', pipe_with_byte(b'K'))]);
}

#[test]
fn multiple_inherited_fds_survive_sandbox_wrapper() {
    run_probe_with_fd_pairs(vec![
        (b'A', pipe_with_byte(b'A')),
        (b'B', pipe_with_byte(b'B')),
        (b'C', pipe_with_byte(b'C')),
    ]);
}

// ── cross-sandbox IPC via socketpair ─────────────────────────
//
// Two sandboxed probes share a UnixDatagram::pair. The writer
// sends a marker byte; the reader verifies it arrives intact.
// Mirrors the production capsa-netd / capsa-vmm fd chain topology.

const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const MARKER: &str = "X";

#[test]
fn byte_traverses_two_concurrent_sandboxes_via_socketpair() {
    let (writer_end, reader_end) = UnixDatagram::pair().expect("unix datagram pair");
    let writer_owned: OwnedFd = writer_end.into();
    let reader_owned: OwnedFd = reader_end.into();

    let probe = probe_binary();

    let mut writer_builder = common::sandbox_builder();
    let writer_raw = writer_builder.inherit_fd(writer_owned);
    let mut writer_cmd = writer_builder
        .command(&probe)
        .expect("build writer sandbox");
    writer_cmd
        .arg("fd-write-byte")
        .arg(writer_raw.to_string())
        .arg(MARKER)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut reader_builder = common::sandbox_builder();
    let reader_raw = reader_builder.inherit_fd(reader_owned);
    let mut reader_cmd = reader_builder
        .command(&probe)
        .expect("build reader sandbox");
    reader_cmd
        .arg("fd-read-byte")
        .arg(reader_raw.to_string())
        .arg(MARKER)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut writer_child = writer_cmd.spawn().expect("spawn writer probe");
    let mut reader_child = reader_cmd.spawn().expect("spawn reader probe");

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

fn wait_within(child: &mut SandboxChild, timeout: Duration, label: &str) -> ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
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

// ── fd sealing ──────────────────────────────────────────────

#[test]
fn non_inherited_fd_is_sealed_in_child() {
    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(b"Z").expect("write marker");
    drop(write_end);

    // Move the pipe to a high fd number so it doesn't collide with
    // fds that sandbox-exec opens internally (it reuses low numbers
    // like 3 for its own sockets).
    let original: OwnedFd = read_end.into();
    let high_raw = 100;
    assert_ne!(
        unsafe { libc::dup2(original.as_raw_fd(), high_raw) },
        -1,
        "dup2 to fd {high_raw} failed"
    );
    drop(original);
    let leak_fd = unsafe { OwnedFd::from_raw_fd(high_raw) };
    let leak_raw = leak_fd.as_raw_fd();

    // Keep the fd alive in the parent but do NOT register it via
    // inherit_fd. seal_fds (called by build) should close it in the
    // child, causing fd-read-byte to fail with EBADF.
    let probe = probe_binary();
    let builder = common::sandbox_builder();
    let mut cmd = builder.command(&probe).expect("build sandbox");
    cmd.arg("fd-read-byte")
        .arg(leak_raw.to_string())
        .arg("Z")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");

    assert!(
        !output.status.success(),
        "probe should have failed reading non-inherited fd {leak_raw}, \
         but exited successfully; seal_fds did not close it"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_msg = format!("fcntl(F_GETFD) on fd {leak_raw} failed");
    assert!(
        stderr.contains(&expected_msg),
        "expected EBADF error for fd {leak_raw} in stderr, got: {stderr}"
    );

    drop(leak_fd);
}

#[test]
fn inherited_fd_survives_seal() {
    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(b"Q").expect("write marker");
    drop(write_end);

    let read_owned: OwnedFd = read_end.into();
    let read_raw = read_owned.as_raw_fd();

    let probe = probe_binary();
    let mut builder = common::sandbox_builder();
    builder.inherit_fd(read_owned);
    let mut cmd = builder.command(&probe).expect("build sandbox");
    cmd.arg("fd-read-byte")
        .arg(read_raw.to_string())
        .arg("Q")
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd.status().expect("spawn probe");
    assert!(
        status.success(),
        "inherited fd {read_raw} should survive seal; probe exited with {status:?}"
    );
}
