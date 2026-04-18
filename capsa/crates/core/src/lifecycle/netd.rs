//! Shared helpers for bringing up a `capsa-netd` child: the sandbox
//! policy builder, the readiness-timeout constant, and the async
//! `wait_ready` that consumes the daemon's one-byte handshake.

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};
use lockin::SandboxBuilder;
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;

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

/// Await netd's one-byte readiness signal, or time out. The pipe
/// read end is wrapped in [`AsyncFd`] so the wait does not block a
/// thread; a [`tokio::time::timeout`] enforces the overall deadline.
pub(super) async fn wait_ready(reader: OwnedFd, timeout: Duration) -> Result<()> {
    set_nonblocking(reader.as_raw_fd()).context("set readiness fd nonblocking")?;
    let async_fd =
        AsyncFd::with_interest(reader, Interest::READABLE).context("wrap readiness fd")?;

    let read_fut = read_ready_byte(&async_fd);
    match tokio::time::timeout(timeout, read_fut).await {
        Ok(result) => result,
        Err(_) => bail!("timed out waiting for net daemon readiness signal"),
    }
}

async fn read_ready_byte(async_fd: &AsyncFd<OwnedFd>) -> Result<()> {
    let signal = loop {
        let mut guard = async_fd
            .readable()
            .await
            .context("wait readable on readiness fd")?;
        match guard.try_io(|inner| {
            let mut buf = [0u8; 1];
            // SAFETY: read(2) on a valid fd with a valid buffer is
            // well-defined. We read exactly one byte to satisfy the
            // readiness protocol.
            let rc = unsafe { libc::read(inner.get_ref().as_raw_fd(), buf.as_mut_ptr().cast(), 1) };
            if rc < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if rc == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "failed reading net daemon readiness byte: pipe closed",
                ));
            }
            Ok(buf[0])
        }) {
            Ok(Ok(byte)) => break byte,
            Ok(Err(err)) => return Err(err).context("failed reading net daemon readiness byte"),
            Err(_would_block) => continue,
        }
    };

    ensure!(
        signal == READY_SIGNAL,
        "invalid net daemon readiness byte: expected {:?}, got {:?}",
        READY_SIGNAL,
        signal
    );

    Ok(())
}

fn set_nonblocking(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    // SAFETY: F_GETFL/F_SETFL on a valid fd is well-defined.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let rc = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_ready_accepts_correct_signal() {
        let reader = pipe_with_byte(READY_SIGNAL);
        wait_ready(reader, Duration::from_secs(1))
            .await
            .expect("correct byte should succeed");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_ready_rejects_wrong_byte() {
        let reader = pipe_with_byte(b'X');
        let err = wait_ready(reader, Duration::from_secs(1))
            .await
            .expect_err("wrong byte should fail");
        assert!(
            err.to_string()
                .contains("invalid net daemon readiness byte"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_ready_fails_on_closed_pipe() {
        let (read_end, write_end) = std::io::pipe().expect("create pipe");
        drop(write_end);
        let err = wait_ready(read_end.into(), Duration::from_secs(1))
            .await
            .expect_err("closed pipe should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("failed reading") || msg.contains("pipe closed"),
            "unexpected error: {err}"
        );
    }
}
