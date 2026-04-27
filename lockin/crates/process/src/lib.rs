#![doc = include_str!("../README.md")]

use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Extension trait for [`Command`] that provides fd inheritance and
/// sealing for child processes.
///
/// Each method installs its own `pre_exec` hook. Hooks run in
/// registration order, so call [`seal_fds`](CommandFdExt::seal_fds)
/// **before** [`map_fd`](CommandFdExt::map_fd) /
/// [`keep_fd`](CommandFdExt::keep_fd). The seal sweeps all fds `>= 3`
/// to `FD_CLOEXEC` at exec time; subsequent map/keep hooks clear
/// `FD_CLOEXEC` on the fds the child should keep.
pub trait CommandFdExt {
    fn map_fd(&mut self, fd: OwnedFd, child_fd: RawFd) -> &mut Self;
    fn keep_fd(&mut self, fd: OwnedFd) -> &mut Self;

    /// Registers a `pre_exec` hook that sweeps all fds `>= 3` to
    /// `FD_CLOEXEC` at exec time, so any fd not whitelisted by a
    /// later [`map_fd`](Self::map_fd) / [`keep_fd`](Self::keep_fd)
    /// hook is closed by the kernel on `execve`. The sweep runs
    /// child-side after `fork`, so fds opened in the parent between
    /// this call and `spawn` are still covered.
    fn seal_fds(&mut self) -> &mut Self;
}

/// Upper bound on the cloexec sweep range. `getrlimit(RLIMIT_NOFILE)`
/// can return `RLIM_INFINITY` or extremely large values; iterating
/// billions of fds inside `pre_exec` would stall the child.
const MAX_FD_SWEEP: u64 = 65_536;

fn parent_max_fd() -> RawFd {
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit reads into a stack-local struct.
    let rc = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) };
    let lim = if rc == 0 { rlim.rlim_cur } else { 1024 };
    std::cmp::min(lim, MAX_FD_SWEEP as libc::rlim_t) as RawFd
}

/// async-signal-safe: only invokes `fcntl`. `EBADF` on holes in the
/// fd table is harmless.
unsafe fn fcntl_cloexec_sweep(max_fd: RawFd) {
    let mut fd: RawFd = 3;
    while fd <= max_fd {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags != -1 {
            libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
        }
        fd += 1;
    }
}

#[cfg(target_os = "linux")]
unsafe fn cloexec_sweep(max_fd: RawFd) {
    // close_range(2) with CLOSE_RANGE_CLOEXEC marks CLOEXEC across
    // the whole range in a single syscall (Linux 5.11+). On older
    // kernels (ENOSYS) or kernels that recognise the syscall but not
    // the flag (EINVAL), fall back to a fcntl loop.
    let rc = libc::syscall(
        libc::SYS_close_range,
        3 as libc::c_uint,
        !0u32 as libc::c_uint,
        libc::CLOSE_RANGE_CLOEXEC,
    );
    if rc == 0 {
        return;
    }
    fcntl_cloexec_sweep(max_fd);
}

#[cfg(not(target_os = "linux"))]
unsafe fn cloexec_sweep(max_fd: RawFd) {
    fcntl_cloexec_sweep(max_fd);
}

impl CommandFdExt for Command {
    fn map_fd(&mut self, fd: OwnedFd, child_fd: RawFd) -> &mut Self {
        assert!(child_fd >= 3, "map_fd: child_fd {child_fd} must be >= 3");

        let src_fd = fd.as_raw_fd();

        // SAFETY: dup2 and fcntl are async-signal-safe per POSIX. The
        // OwnedFd is moved into the closure to keep it alive in the
        // parent until spawn.
        unsafe {
            self.pre_exec(move || {
                let _keep_alive = &fd;

                if src_fd != child_fd && libc::dup2(src_fd, child_fd) == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                let flags = libc::fcntl(child_fd, libc::F_GETFD);
                if flags == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(child_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                Ok(())
            });
        }

        self
    }

    fn keep_fd(&mut self, fd: OwnedFd) -> &mut Self {
        let raw = fd.as_raw_fd();
        self.map_fd(fd, raw)
    }

    fn seal_fds(&mut self) -> &mut Self {
        let max_fd = parent_max_fd();

        // SAFETY: close_range and fcntl are async-signal-safe. The
        // sweep runs child-side after fork, so fds opened in the
        // parent between this call and spawn are still covered.
        unsafe {
            self.pre_exec(move || {
                cloexec_sweep(max_fd);
                Ok(())
            });
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn map_fd_remaps_to_target_number() {
        let (read_end, mut write_end) = std::io::pipe().expect("pipe");
        writeln!(write_end, "hello").expect("write");
        drop(write_end);

        let read_owned: OwnedFd = read_end.into();
        let target_fd: RawFd = 10;

        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "IFS= read -r line < /dev/fd/{target_fd}; [ \"$line\" = \"hello\" ]"
            ))
            .map_fd(read_owned, target_fd)
            .status()
            .expect("spawn");

        assert!(
            status.success(),
            "child should read 'hello' from fd {target_fd}"
        );
    }

    #[test]
    fn keep_fd_preserves_current_number() {
        let (read_end, mut write_end) = std::io::pipe().expect("pipe");
        writeln!(write_end, "world").expect("write");
        drop(write_end);

        let read_owned: OwnedFd = read_end.into();
        let raw = read_owned.as_raw_fd();

        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "IFS= read -r line < /dev/fd/{raw}; [ \"$line\" = \"world\" ]"
            ))
            .keep_fd(read_owned)
            .status()
            .expect("spawn");

        assert!(status.success(), "child should read 'world' from fd {raw}");
    }

    #[test]
    fn seal_closes_unregistered_fds() {
        let (read_end, mut write_end) = std::io::pipe().expect("pipe");
        writeln!(write_end, "sealed").expect("write");
        drop(write_end);

        let leaked: OwnedFd = read_end.into();
        let leaked_raw = leaked.as_raw_fd();

        // seal_fds marks everything FD_CLOEXEC. We do NOT call keep_fd,
        // so the fd should be closed at exec.
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "IFS= read -r line < /dev/fd/{leaked_raw} 2>/dev/null; [ $? -ne 0 ]"
            ))
            .seal_fds()
            .status()
            .expect("spawn");

        drop(leaked);

        assert!(
            status.success(),
            "child should NOT be able to read from sealed fd {leaked_raw}"
        );
    }

    #[test]
    fn seal_then_keep_preserves_fd() {
        let (read_end, mut write_end) = std::io::pipe().expect("pipe");
        writeln!(write_end, "kept").expect("write");
        drop(write_end);

        let read_owned: OwnedFd = read_end.into();
        let raw = read_owned.as_raw_fd();

        // seal_fds first (sets FD_CLOEXEC on everything), then keep_fd
        // (clears FD_CLOEXEC on this fd). Hooks run in registration order.
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "IFS= read -r line < /dev/fd/{raw}; [ \"$line\" = \"kept\" ]"
            ))
            .seal_fds()
            .keep_fd(read_owned)
            .status()
            .expect("spawn");

        assert!(status.success(), "kept fd {raw} should survive seal");
    }

    #[test]
    fn seal_then_map_preserves_remapped_fd() {
        let (read_end, mut write_end) = std::io::pipe().expect("pipe");
        writeln!(write_end, "remapped").expect("write");
        drop(write_end);

        let read_owned: OwnedFd = read_end.into();
        let target_fd: RawFd = 15;

        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "IFS= read -r line < /dev/fd/{target_fd}; [ \"$line\" = \"remapped\" ]"
            ))
            .seal_fds()
            .map_fd(read_owned, target_fd)
            .status()
            .expect("spawn");

        assert!(status.success(), "mapped fd should survive seal");
    }
}
