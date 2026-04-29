//! Best-effort foreground-tty acquisition for the *current* process.
//!
//! The same SIGTTOU/setpgid/tcsetpgrp dance that
//! [`crate::supervise::supervise_command`] performs on behalf of a
//! sandboxed child, but applied to the calling process itself. Used
//! by callers that hand off the controlling tty to libraries that
//! call `tcsetattr` to put the terminal in raw mode (libkrun, custom
//! TUI hosts) — `tcsetattr` only succeeds when the calling pgrp is
//! the foreground pgrp of the controlling tty.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// Claim foreground-pgrp ownership of the controlling tty for the
/// current process. The returned guard restores the previous
/// foreground pgrp and the saved SIGTTOU/SIGTTIN dispositions on
/// `Drop`.
///
/// Steps:
/// 1. Save and replace SIGTTOU/SIGTTIN with `SIG_IGN` so the
///    `tcsetpgrp` calls below can't suspend us.
/// 2. Open `/dev/tty`. If the process has no controlling tty, return
///    a no-op guard — non-interactive runs (CI, redirected stdin)
///    have nothing to do.
/// 3. `tcgetpgrp` to record the original foreground pgrp.
/// 4. `setpgid(0, 0)` (best-effort — `EPERM` if we're already a pgrp
///    leader is benign).
/// 5. `tcsetpgrp(/dev/tty, getpid())` to claim the foreground.
pub fn claim_foreground_tty() -> io::Result<ForegroundTtyGuard> {
    let saved_sigttou = ignore_signal(libc::SIGTTOU)?;
    let saved_sigttin = match ignore_signal(libc::SIGTTIN) {
        Ok(s) => s,
        Err(err) => {
            restore_signal(libc::SIGTTOU, &saved_sigttou);
            return Err(err);
        }
    };

    let tty_fd = open_controlling_tty();
    let mut original_pgrp: libc::pid_t = -1;

    if let Some(fd) = tty_fd.as_ref() {
        original_pgrp = unsafe { libc::tcgetpgrp(fd.as_raw_fd()) };
        unsafe { libc::setpgid(0, 0) };
        let pid = unsafe { libc::getpid() };
        if unsafe { libc::tcsetpgrp(fd.as_raw_fd(), pid) } == -1 {
            let err = io::Error::last_os_error();
            restore_signal(libc::SIGTTOU, &saved_sigttou);
            restore_signal(libc::SIGTTIN, &saved_sigttin);
            return Err(err);
        }
    }

    Ok(ForegroundTtyGuard {
        tty_fd,
        original_pgrp,
        saved_sigttou,
        saved_sigttin,
    })
}

/// On `Drop`, restores the foreground pgrp recorded at acquisition
/// and the saved SIGTTOU/SIGTTIN handlers. Held by the caller for
/// the duration of the work that needed foreground-tty ownership.
pub struct ForegroundTtyGuard {
    tty_fd: Option<OwnedFd>,
    original_pgrp: libc::pid_t,
    saved_sigttou: libc::sigaction,
    saved_sigttin: libc::sigaction,
}

impl Drop for ForegroundTtyGuard {
    fn drop(&mut self) {
        if let Some(fd) = self.tty_fd.as_ref() {
            if self.original_pgrp > 0 {
                unsafe { libc::tcsetpgrp(fd.as_raw_fd(), self.original_pgrp) };
            }
        }
        restore_signal(libc::SIGTTOU, &self.saved_sigttou);
        restore_signal(libc::SIGTTIN, &self.saved_sigttin);
    }
}

pub(crate) fn open_controlling_tty() -> Option<OwnedFd> {
    let fd = unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 {
        None
    } else {
        Some(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

pub(crate) fn ignore_signal(sig: libc::c_int) -> io::Result<libc::sigaction> {
    let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
    let mut new: libc::sigaction = unsafe { std::mem::zeroed() };
    new.sa_sigaction = libc::SIG_IGN;
    if unsafe { libc::sigaction(sig, &new, &mut old) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(old)
}

pub(crate) fn restore_signal(sig: libc::c_int, prev: &libc::sigaction) {
    if unsafe { libc::sigaction(sig, prev, std::ptr::null_mut()) } != 0 {
        eprintln!(
            "lockin: failed to restore signal {sig} disposition: {}",
            io::Error::last_os_error()
        );
    }
}
