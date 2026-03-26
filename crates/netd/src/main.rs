use std::fs::File;
use std::io::{self, Write};
use std::os::fd::FromRawFd;

use anyhow::{Context, Result};

use capsa_core::daemon::constants::NETD_READY_FD;
use capsa_core::daemon::net::args::parse_launch_spec_args;

const READY_SIGNAL: u8 = b'R';

fn run<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run_with_ready_fd(args, NETD_READY_FD)
}

fn run_with_ready_fd<I, S>(args: I, ready_fd: i32) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let launch_spec = parse_launch_spec_args(args)?;
    launch_spec
        .validate()
        .context("invalid net daemon launch spec")?;

    signal_readiness(ready_fd).context("failed to signal net daemon readiness")?;

    Ok(())
}

fn signal_readiness(ready_fd: i32) -> io::Result<()> {
    if ready_fd < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid readiness fd: {ready_fd}"),
        ));
    }

    // SAFETY: `ready_fd` is provided by the launcher as a valid writable file descriptor,
    // and ownership is transferred to this function. `File::from_raw_fd` takes ownership
    // and closes the descriptor on drop.
    let mut ready_file = unsafe { File::from_raw_fd(ready_fd) };
    ready_file.write_all(&[READY_SIGNAL])?;
    ready_file.flush()?;

    Ok(())
}

fn main() -> Result<()> {
    run(std::env::args().skip(1))
}

#[cfg(test)]
mod tests {
    use super::{run_with_ready_fd, signal_readiness, READY_SIGNAL};
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::FromRawFd;

    fn sample_launch_spec_json() -> String {
        serde_json::json!({
            "interfaces": [
                {
                    "host_fd": 200,
                    "mac": [2, 170, 187, 204, 221, 238],
                    "policy": null
                }
            ]
        })
        .to_string()
    }

    fn pipe() -> (File, i32) {
        let mut fds = [0; 2];
        // SAFETY: `fds` points to valid memory for two integers.
        let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "pipe creation must succeed");

        // SAFETY: `pipe` filled `fds[0]` with a valid read descriptor.
        let reader = unsafe { File::from_raw_fd(fds[0]) };
        (reader, fds[1])
    }

    #[test]
    fn argument_parsing_success_path() {
        let (mut reader, writer_fd) = pipe();

        run_with_ready_fd(
            vec!["--launch-spec-json".to_string(), sample_launch_spec_json()],
            writer_fd,
        )
        .expect("valid args should succeed");

        let mut buf = [0u8; 1];
        reader
            .read_exact(&mut buf)
            .expect("readiness byte should be available");
        assert_eq!(buf[0], READY_SIGNAL);
    }

    #[test]
    fn argument_parsing_failure_path() {
        let err = run_with_ready_fd(vec!["--bad-flag".to_string()], -1)
            .expect_err("invalid args should fail before readiness signaling");
        assert_eq!(
            err.to_string(),
            "usage: capsa-netd --launch-spec-json <json>"
        );
    }

    #[test]
    fn malformed_launch_spec_json_returns_parse_error_before_readiness() {
        let err = run_with_ready_fd(
            vec!["--launch-spec-json".to_string(), "{not-json".to_string()],
            -1,
        )
        .expect_err("malformed launch spec json should fail");

        let message = err.to_string();
        assert!(message.contains("failed to parse net daemon launch spec JSON"));
        assert!(!message.contains("failed to signal net daemon readiness"));
    }

    #[test]
    fn invalid_launch_spec_returns_validation_error_before_readiness() {
        let invalid_spec_json = serde_json::json!({
            "interfaces": [
                {
                    "host_fd": 200,
                    "mac": [2, 170, 187, 204, 221, 238],
                    "policy": null
                },
                {
                    "host_fd": 200,
                    "mac": [2, 170, 187, 204, 221, 239],
                    "policy": null
                }
            ]
        })
        .to_string();

        let err = run_with_ready_fd(
            vec!["--launch-spec-json".to_string(), invalid_spec_json],
            -1,
        )
        .expect_err("invalid launch spec should fail");

        let message = err.to_string();
        assert!(message.contains("invalid net daemon launch spec"));
        assert!(!message.contains("failed to signal net daemon readiness"));
    }

    #[test]
    fn readiness_helper_writes_exact_ready_byte() {
        let (mut reader, writer_fd) = pipe();

        signal_readiness(writer_fd).expect("readiness signaling should succeed");

        let mut buf = [0u8; 1];
        reader
            .read_exact(&mut buf)
            .expect("readiness byte should be readable");
        assert_eq!(buf, [READY_SIGNAL]);
    }

    #[test]
    fn readiness_helper_rejects_invalid_fd() {
        let err = signal_readiness(-1).expect_err("negative fd should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
