//! Inherited file descriptor contract: fds registered via
//! `SandboxBuilder::inherit_fd` survive the sandbox wrapper exec
//! (sandbox-exec on macOS, syd on Linux), and two independently
//! sandboxed children can exchange data through a shared kernel
//! object (UnixDatagram socketpair).

mod common;

use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::process::{ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use capsa_sandbox::Sandbox;

use common::{probe_binary, ChildGuard};

// ── single-process fd inheritance ────────────────────────────

fn pipe_with_byte(byte: u8) -> OwnedFd {
    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(&[byte]).expect("write marker byte");
    drop(write_end);
    read_end.into()
}

fn run_probe_with_fd_pairs(pairs: Vec<(u8, OwnedFd)>) {
    let mut builder = Sandbox::builder();
    let mut pair_args: Vec<String> = Vec::with_capacity(pairs.len() * 2);
    for (expected_byte, owned) in pairs {
        let pre_raw = owned.as_raw_fd();
        let raw = builder
            .inherit_fd(owned)
            .unwrap_or_else(|e| panic!("inherit_fd({pre_raw}): {e}"));
        assert_eq!(raw, pre_raw);
        pair_args.push(raw.to_string());
        pair_args.push(std::str::from_utf8(&[expected_byte]).unwrap().to_string());
    }

    let probe = probe_binary();
    let (mut cmd, _sandbox) = builder.build(&probe).expect("build sandbox for probe");
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
