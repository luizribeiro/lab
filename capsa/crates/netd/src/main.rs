use anyhow::{Context, Result};

use capsa_spec::{parse_launch_spec_args, NetLaunchSpec};

mod control;
mod runtime;

fn run<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let launch_spec: NetLaunchSpec = parse_launch_spec_args(args)?;
    launch_spec
        .validate()
        .context("invalid net daemon launch spec")?;

    let ready_fd = launch_spec.ready_fd;

    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for net daemon")?;

    tokio_runtime.block_on(runtime::run(launch_spec, ready_fd))
}

fn main() -> Result<()> {
    run(std::env::args().skip(1))
}

#[cfg(test)]
mod tests {
    use super::run;

    fn sample_launch_spec_json(ready_fd: i32) -> String {
        serde_json::json!({
            "ready_fd": ready_fd,
            "control_fd": null,
        })
        .to_string()
    }

    #[test]
    fn argument_parsing_failure_path() {
        let err = run(vec!["--bad-flag".to_string()])
            .expect_err("invalid args should fail before runtime startup");
        assert_eq!(err.to_string(), "usage: --launch-spec-json <json>");
    }

    #[test]
    fn malformed_launch_spec_json_returns_parse_error_before_runtime() {
        let err = run(vec![
            "--launch-spec-json".to_string(),
            "{not-json".to_string(),
        ])
        .expect_err("malformed launch spec json should fail");

        assert!(err.to_string().contains("failed to parse launch spec JSON"));
    }

    #[test]
    fn invalid_launch_spec_returns_validation_error_before_runtime() {
        let invalid_spec_json = serde_json::json!({
            "ready_fd": 30,
            "control_fd": 30,
            "policy": null,
        })
        .to_string();

        let err = run(vec!["--launch-spec-json".to_string(), invalid_spec_json])
            .expect_err("invalid launch spec should fail");

        assert!(err.to_string().contains("invalid net daemon launch spec"));
    }

    #[test]
    fn valid_launch_spec_propagates_readiness_fd_error() {
        // Open /dev/null read-only and pass that fd as ready_fd. It
        // passes spec validation (a real, non-negative fd >= 3) but
        // writing the readiness byte will fail with EBADF since the
        // fd is read-only. This exercises the "failed to signal net
        // daemon readiness" error-wrapping path.
        // SAFETY: open on /dev/null with O_RDONLY is well-defined.
        let ready_fd =
            unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        assert!(ready_fd >= 3, "/dev/null open should yield fd >= 3");

        let err = run(vec![
            "--launch-spec-json".to_string(),
            sample_launch_spec_json(ready_fd),
        ])
        .expect_err("runtime should fail writing readiness byte on read-only fd");

        assert!(
            err.to_string()
                .contains("failed to signal net daemon readiness"),
            "unexpected error: {err}"
        );
    }
}
