use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Extension trait for [`Command`] that provides fd inheritance and
/// sealing for child processes.
///
/// Each method installs its own `pre_exec` hook. Hooks run in
/// registration order, so call [`seal_fds`](CommandFdExt::seal_fds)
/// **before** [`map_fd`](CommandFdExt::map_fd) /
/// [`keep_fd`](CommandFdExt::keep_fd). The seal marks everything
/// `FD_CLOEXEC`; subsequent map/keep hooks clear `FD_CLOEXEC` on the
/// fds you want the child to keep.
pub trait CommandFdExt {
    fn map_fd(&mut self, fd: OwnedFd, child_fd: RawFd) -> &mut Self;
    fn keep_fd(&mut self, fd: OwnedFd) -> &mut Self;
    fn seal_fds(&mut self) -> &mut Self;
}

fn enumerate_open_fds() -> Vec<RawFd> {
    let fd_dir = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };

    let entries = match std::fs::read_dir(fd_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    entries
        .filter_map(|entry| {
            let name = entry.ok()?.file_name();
            let fd: RawFd = name.to_str()?.parse().ok()?;
            if fd >= 3 {
                Some(fd)
            } else {
                None
            }
        })
        .collect()
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
        let fds_to_seal = enumerate_open_fds();

        // SAFETY: fcntl with F_SETFD is async-signal-safe per POSIX.
        // EBADF on fds closed between snapshot and fork is harmless.
        unsafe {
            self.pre_exec(move || {
                for &fd in &fds_to_seal {
                    libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
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
                "IFS= read -r line <&{target_fd}; [ \"$line\" = \"hello\" ]"
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
                "IFS= read -r line <&{raw}; [ \"$line\" = \"world\" ]"
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
                "IFS= read -r line <&{leaked_raw} 2>/dev/null; [ $? -ne 0 ]"
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
                "IFS= read -r line <&{raw}; [ \"$line\" = \"kept\" ]"
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
                "IFS= read -r line <&{target_fd}; [ \"$line\" = \"remapped\" ]"
            ))
            .seal_fds()
            .map_fd(read_owned, target_fd)
            .status()
            .expect("spawn");

        assert!(status.success(), "mapped fd should survive seal");
    }
}
