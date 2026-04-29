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
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::process::{ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use lockin::SandboxedChild;

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

fn wait_within(child: &mut SandboxedChild, timeout: Duration, label: &str) -> ExitStatus {
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
fn fd_opened_after_command_construction_is_sealed() {
    // Regression test: seal_fds must close fds that didn't exist
    // when builder.command() returned. A parent-side snapshot taken
    // at command-construction time would miss this fd; the
    // child-side cloexec sweep at exec time catches it.
    let probe = probe_binary();
    let builder = common::sandbox_builder();
    let mut cmd = builder.command(&probe).expect("build sandbox");

    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(b"L").expect("write marker");
    drop(write_end);

    // dup2 to a high fd so FD_CLOEXEC (set by std on pipe creation)
    // is cleared. Without this, the fd would close at exec naturally
    // and we wouldn't be testing seal_fds at all.
    let original: OwnedFd = read_end.into();
    let high_raw = 101;
    assert_ne!(
        unsafe { libc::dup2(original.as_raw_fd(), high_raw) },
        -1,
        "dup2 to fd {high_raw} failed"
    );
    drop(original);
    let leak_fd = unsafe { OwnedFd::from_raw_fd(high_raw) };
    let leak_raw = leak_fd.as_raw_fd();

    cmd.arg("fd-read-byte")
        .arg(leak_raw.to_string())
        .arg("L")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");

    assert!(
        !output.status.success(),
        "probe should have failed reading post-construction fd {leak_raw}, \
         but exited successfully; seal_fds did not seal it"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_msg = format!("fcntl(F_GETFD) on fd {leak_raw} failed");
    assert!(
        stderr.contains(&expected_msg),
        "expected EBADF error for fd {leak_raw} in stderr, got: {stderr}"
    );

    drop(leak_fd);
}

#[cfg(target_os = "linux")]
#[test]
fn high_numbered_fd_above_cap_is_sealed_in_child() {
    // Regression test for the macOS MAX_FD_SWEEP cap (65,536).
    // Pre-fix, fds above the cap were never marked CLOEXEC because
    // the child-side fcntl sweep stopped at 65,536 and there was no
    // parent-side enumeration. Tries to open an fd at number 70,000
    // and asserts the sandboxed child cannot read it.
    //
    // Self-skips (with a printed reason) if the test environment's
    // RLIMIT_NOFILE hard limit doesn't allow raising the soft limit
    // that high.
    //
    // On macOS, kern.maxfilesperproc prevents dup2 to fd numbers above
    // ~24K regardless of RLIMIT_NOFILE; the equivalent regression
    // scenario can't be constructed in the test environment, so this
    // test is Linux-only. The macOS code path (proc_pidinfo
    // enumeration) is exercised by the other fd_inheritance tests at
    // lower fd numbers.
    let needed: libc::rlim_t = 70_001;
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) } != 0 {
        eprintln!("skipping high_numbered_fd_above_cap_is_sealed_in_child: getrlimit failed");
        return;
    }
    if rlim.rlim_max < needed {
        eprintln!(
            "skipping high_numbered_fd_above_cap_is_sealed_in_child: \
             RLIMIT_NOFILE hard limit is {} < {needed}",
            rlim.rlim_max
        );
        return;
    }
    rlim.rlim_cur = needed;
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) } != 0 {
        eprintln!("skipping high_numbered_fd_above_cap_is_sealed_in_child: setrlimit failed");
        return;
    }

    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(b"X").expect("write marker");
    drop(write_end);

    let original: OwnedFd = read_end.into();
    let high_raw: i32 = 70_000;
    assert_ne!(
        unsafe { libc::dup2(original.as_raw_fd(), high_raw) },
        -1,
        "dup2 to fd {high_raw} failed"
    );
    drop(original);
    let leak_fd = unsafe { OwnedFd::from_raw_fd(high_raw) };

    let probe = probe_binary();
    let builder = common::sandbox_builder();
    let mut cmd = builder.command(&probe).expect("build sandbox");
    cmd.arg("fd-read-byte")
        .arg(high_raw.to_string())
        .arg("X")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "probe read from fd {high_raw} successfully — the cap bug is back"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_msg = format!("fcntl(F_GETFD) on fd {high_raw} failed");
    assert!(
        stderr.contains(&expected_msg),
        "expected EBADF error for fd {high_raw} in stderr, got: {stderr}"
    );

    drop(leak_fd);
}

// ── inherit_fd_as: explicit child fd number ──────────────────

#[test]
fn mapped_fd_appears_at_requested_child_fd() {
    let owned = pipe_with_byte(b'M');
    let target_fd = 3;

    let probe = probe_binary();
    let builder = common::sandbox_builder().inherit_fd_as(owned, target_fd);
    let mut cmd = builder.command(&probe).expect("build sandbox");
    cmd.arg("fd-read-byte")
        .arg(target_fd.to_string())
        .arg("M")
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd.status().expect("spawn probe");
    assert!(
        status.success(),
        "child should read 'M' from mapped fd {target_fd}, got {status:?}"
    );
}

#[test]
fn unmapped_unrelated_fd_is_still_sealed_when_inherit_fd_as_used() {
    // inherit_fd_as on one fd must not leak some unrelated high fd.
    let mapped = pipe_with_byte(b'A');

    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(b"Z").expect("write marker");
    drop(write_end);
    let original: OwnedFd = read_end.into();
    let leak_raw = 102;
    assert_ne!(
        unsafe { libc::dup2(original.as_raw_fd(), leak_raw) },
        -1,
        "dup2 to fd {leak_raw} failed"
    );
    drop(original);
    let leak_fd = unsafe { OwnedFd::from_raw_fd(leak_raw) };

    let probe = probe_binary();
    let builder = common::sandbox_builder().inherit_fd_as(mapped, 3);
    let mut cmd = builder.command(&probe).expect("build sandbox");
    cmd.arg("fd-read-byte")
        .arg(leak_raw.to_string())
        .arg("Z")
        .stderr(Stdio::piped())
        .stdout(Stdio::inherit());

    let output = cmd.output().expect("spawn probe");
    assert!(
        !output.status.success(),
        "leaked fd {leak_raw} should be sealed even when inherit_fd_as is used"
    );

    drop(leak_fd);
}

#[test]
fn mapped_fd_does_not_collide_with_existing_inherit_fd() {
    // inherit_fd (keeps original number) and inherit_fd_as (maps to 3)
    // should both work in the same builder. Relocate the kept fd to
    // a number >= 10 so it cannot accidentally land on the target fd
    // when the test runs in an environment with no extra fds open
    // (clean CI runners allocate pipes starting at fd 3).
    let kept_initial = pipe_with_byte(b'K');
    let mapped = pipe_with_byte(b'P');
    let kept_initial_raw = kept_initial.into_raw_fd();
    let kept_high = unsafe { libc::fcntl(kept_initial_raw, libc::F_DUPFD_CLOEXEC, 10) };
    assert!(
        kept_high >= 10,
        "F_DUPFD_CLOEXEC failed: rc={kept_high}, errno={}",
        std::io::Error::last_os_error()
    );
    unsafe { libc::close(kept_initial_raw) };
    let kept = unsafe { OwnedFd::from_raw_fd(kept_high) };
    let kept_raw = kept_high;
    let target_fd = 3;

    let probe = probe_binary();
    let mut builder = common::sandbox_builder();
    builder.inherit_fd(kept);
    let builder = builder.inherit_fd_as(mapped, target_fd);
    let mut cmd = builder.command(&probe).expect("build sandbox");
    cmd.arg("fd-read-byte")
        .arg(kept_raw.to_string())
        .arg("K")
        .arg(target_fd.to_string())
        .arg("P")
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd.status().expect("spawn probe");
    assert!(
        status.success(),
        "both fds should arrive: kept at {kept_raw}, mapped at {target_fd}; got {status:?}"
    );
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
