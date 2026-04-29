//! Regression test: `lockin::fg_tty::claim_foreground_tty` must
//! transition the calling process from a background pgrp on the
//! controlling tty to the foreground pgrp. Without this property,
//! libkrun's `tcsetattr` later fails with EPERM and the VM console
//! is left in line-buffered/double-echo mode.
//!
//! Setup:
//! 1. `forkpty(3)` creates a fresh pty whose slave becomes our
//!    forkpty-child's controlling tty; the forkpty-child is the
//!    session leader and (initially) the foreground pgrp.
//! 2. The forkpty-child `fork(2)`s a *grandchild* and `execv`s the
//!    probe in it. The grandchild is **not** a session leader, so
//!    `setpgid(0, 0)` inside the probe creates a fresh pgrp that is
//!    by definition a *background* pgrp on the tty.
//! 3. The probe captures `tcgetpgrp` before and after calling
//!    `claim_foreground_tty()`, then prints the values.
//! 4. The test reads the line over the pty master and asserts
//!    `after == probe_pid` (foreground claimed) and
//!    `before != probe_pid` (was background before the call).

#![cfg(unix)]

use std::ffi::CString;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn probe_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_capsa-fg-tty-probe"))
}

#[test]
fn claim_foreground_tty_promotes_background_pgrp_to_foreground() {
    let probe = probe_binary();
    let arg0 = CString::new(probe.as_os_str().as_bytes()).unwrap();
    let argv: Vec<*const libc::c_char> = vec![arg0.as_ptr(), std::ptr::null()];

    let mut master_fd: libc::c_int = 0;
    let pid = unsafe {
        libc::forkpty(
            &mut master_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if pid < 0 {
        panic!("forkpty: {}", std::io::Error::last_os_error());
    }
    if pid == 0 {
        // forkpty-child: session leader of the new pty. Fork once
        // more so the probe runs in a non-session-leader process.
        let grand = unsafe { libc::fork() };
        if grand < 0 {
            unsafe { libc::_exit(40) };
        }
        if grand == 0 {
            unsafe {
                libc::execv(arg0.as_ptr(), argv.as_ptr());
                libc::_exit(127);
            }
        }
        let mut status: libc::c_int = 0;
        unsafe {
            libc::waitpid(grand, &mut status, 0);
        }
        let exit_code = if (status & 0x7f) == 0 {
            (status >> 8) & 0xff
        } else {
            128 + (status & 0x7f)
        };
        unsafe { libc::_exit(exit_code) };
    }

    let master = unsafe { OwnedFd::from_raw_fd(master_fd) };
    let output = read_until_eof_or_deadline(&master, Duration::from_secs(10));

    let mut status: libc::c_int = 0;
    unsafe {
        libc::waitpid(pid, &mut status, 0);
    }

    let line = output
        .lines()
        .find(|l| l.contains("after="))
        .unwrap_or_else(|| panic!("probe did not print expected line; full output: {output:?}"));

    let probe_pid = parse_field(line, "pid=");
    let before = parse_field(line, "before=");
    let after = parse_field(line, "after=");

    assert_eq!(
        after, probe_pid,
        "claim_foreground_tty should leave fg pgrp == probe pid; line={line:?}"
    );
    assert_ne!(
        before, probe_pid,
        "probe should start in a background pgrp (forkpty-child is fg); line={line:?}"
    );
}

fn read_until_eof_or_deadline(master: &OwnedFd, total: Duration) -> String {
    let raw = master.as_raw_fd();
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
    unsafe {
        libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(libc::dup(raw)) };
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        match file.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if String::from_utf8_lossy(&buf).contains("after=") {
                    let mut end_deadline = Instant::now() + Duration::from_millis(200);
                    while Instant::now() < end_deadline {
                        match file.read(&mut tmp) {
                            Ok(0) => break,
                            Ok(n) => {
                                buf.extend_from_slice(&tmp[..n]);
                                end_deadline = Instant::now() + Duration::from_millis(50);
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(20));
                            }
                            Err(_) => break,
                        }
                    }
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn parse_field(line: &str, key: &str) -> i32 {
    let rest = line
        .split(key)
        .nth(1)
        .unwrap_or_else(|| panic!("missing {key} in {line:?}"));
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    token
        .parse()
        .unwrap_or_else(|_| panic!("invalid {key} value in {line:?}"))
}
