//! Test helper binary for the `foreground_pgrp` integration test.
//!
//! Expected to be exec'd by a *grandchild* of `forkpty(3)` so that
//! the running process is **not** the session leader of the pty:
//! 1. Make ourselves a fresh pgroup leader (background pgrp on the tty).
//! 2. Read the current foreground pgrp of `/dev/tty`.
//! 3. Call [`lockin::fg_tty::claim_foreground_tty`].
//! 4. Read the foreground pgrp again.
//! 5. Print `pid=<pid> before=<before> after=<after>` so the parent
//!    test can verify `after == pid`.

use std::os::fd::AsRawFd;

fn main() {
    unsafe {
        libc::setpgid(0, 0);
    }
    let pid = unsafe { libc::getpid() };

    let tty = match open_tty() {
        Some(fd) => fd,
        None => {
            eprintln!("probe: no /dev/tty available");
            std::process::exit(2);
        }
    };
    let before = unsafe { libc::tcgetpgrp(tty.as_raw_fd()) };

    let guard = match lockin::fg_tty::claim_foreground_tty() {
        Ok(g) => g,
        Err(err) => {
            eprintln!("probe: claim_foreground_tty failed: {err}");
            std::process::exit(3);
        }
    };

    let after = unsafe { libc::tcgetpgrp(tty.as_raw_fd()) };
    println!("pid={pid} before={before} after={after}");

    drop(guard);
}

fn open_tty() -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    let fd = unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 {
        None
    } else {
        Some(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) })
    }
}
