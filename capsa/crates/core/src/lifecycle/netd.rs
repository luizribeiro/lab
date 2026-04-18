//! Shared helpers for bringing up a `capsa-netd` child: the sandbox
//! policy builder, the readiness-timeout constant, and the blocking
//! `wait_ready` that consumes the daemon's one-byte handshake.

use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use lockin::SandboxBuilder;

use super::child;
use super::plan;

pub(super) const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const READY_SIGNAL: u8 = b'R';

pub(super) fn netd_sandbox_builder(binary_path: &Path) -> SandboxBuilder {
    let mut builder = lockin::Sandbox::builder()
        .allow_network(true)
        .read_only_path(plan::canonical_or_unchanged(binary_path));
    builder = child::apply_syd_path(builder);
    builder = child::apply_library_dirs(builder);
    for runtime_read_path in capsa_net::runtime_read_paths() {
        builder = builder.read_only_path(PathBuf::from(*runtime_read_path));
    }
    builder
}

/// Block until netd sends the one-byte readiness signal.
/// Deadline-based so `EINTR` retries do not extend the total wait
/// beyond `timeout`.
pub(super) fn wait_ready(reader: OwnedFd, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut poll_fd = libc::pollfd {
        fd: reader.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("timed out waiting for net daemon readiness signal");
        }
        let ms = remaining.as_millis().min(i32::MAX as u128) as i32;

        // SAFETY: `poll_fd` points to a valid `pollfd` on the stack.
        let rc = unsafe { libc::poll(&mut poll_fd as *mut libc::pollfd, 1, ms) };
        if rc == 0 {
            bail!("timed out waiting for net daemon readiness signal");
        }
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err).context("poll on net daemon readiness pipe failed");
        }
        break;
    }

    if (poll_fd.revents & libc::POLLIN) == 0 {
        bail!(
            "net daemon readiness pipe became readable without readiness byte (revents={})",
            poll_fd.revents
        );
    }

    let mut ready_file = std::fs::File::from(reader);
    let mut signal = [0u8; 1];
    ready_file
        .read_exact(&mut signal)
        .context("failed reading net daemon readiness byte")?;

    ensure!(
        signal[0] == READY_SIGNAL,
        "invalid net daemon readiness byte: expected {:?}, got {:?}",
        READY_SIGNAL,
        signal[0]
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipe_with_byte(byte: u8) -> OwnedFd {
        let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
        std::io::Write::write_all(&mut write_end, &[byte]).expect("write byte");
        drop(write_end);
        read_end.into()
    }

    #[test]
    fn wait_ready_accepts_correct_signal() {
        let reader = pipe_with_byte(READY_SIGNAL);
        wait_ready(reader, Duration::from_secs(1)).expect("correct byte should succeed");
    }

    #[test]
    fn wait_ready_rejects_wrong_byte() {
        let reader = pipe_with_byte(b'X');
        let err = wait_ready(reader, Duration::from_secs(1)).expect_err("wrong byte should fail");
        assert!(
            err.to_string()
                .contains("invalid net daemon readiness byte"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wait_ready_fails_on_closed_pipe() {
        let (read_end, write_end) = std::io::pipe().expect("create pipe");
        drop(write_end);
        let err = wait_ready(read_end.into(), Duration::from_secs(1))
            .expect_err("closed pipe should fail");
        let msg = err.to_string();
        // Linux returns POLLHUP (no POLLIN) → hits the revents guard;
        // macOS returns POLLIN|POLLHUP → read_exact sees EOF. Both
        // are correct platform behavior for a closed write end.
        assert!(
            msg.contains("failed reading") || msg.contains("readiness"),
            "unexpected error: {err}"
        );
    }
}
