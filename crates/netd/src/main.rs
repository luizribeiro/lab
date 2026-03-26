use anyhow::{Context, Result};

use capsa_core::daemon::constants::NETD_READY_FD;
use capsa_core::daemon::net::args::parse_launch_spec_args;

mod runtime;

fn run<I, S>(args: I, ready_fd: i32) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let launch_spec = parse_launch_spec_args(args)?;
    launch_spec
        .validate()
        .context("invalid net daemon launch spec")?;

    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for net daemon")?;

    tokio_runtime.block_on(runtime::run(launch_spec, ready_fd))
}

fn main() -> Result<()> {
    run(std::env::args().skip(1), NETD_READY_FD)
}

#[cfg(test)]
mod tests {
    use super::run;

    fn sample_launch_spec_json() -> String {
        serde_json::json!({
            "interfaces": []
        })
        .to_string()
    }

    #[test]
    fn argument_parsing_failure_path() {
        let err = run(vec!["--bad-flag".to_string()], -1)
            .expect_err("invalid args should fail before runtime startup");
        assert_eq!(
            err.to_string(),
            "usage: capsa-netd --launch-spec-json <json>"
        );
    }

    #[test]
    fn malformed_launch_spec_json_returns_parse_error_before_runtime() {
        let err = run(
            vec!["--launch-spec-json".to_string(), "{not-json".to_string()],
            -1,
        )
        .expect_err("malformed launch spec json should fail");

        assert!(err
            .to_string()
            .contains("failed to parse net daemon launch spec JSON"));
    }

    #[test]
    fn invalid_launch_spec_returns_validation_error_before_runtime() {
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

        let err = run(
            vec!["--launch-spec-json".to_string(), invalid_spec_json],
            -1,
        )
        .expect_err("invalid launch spec should fail");

        assert!(err.to_string().contains("invalid net daemon launch spec"));
    }

    #[test]
    fn valid_launch_spec_propagates_readiness_fd_error() {
        let err = run(
            vec!["--launch-spec-json".to_string(), sample_launch_spec_json()],
            -1,
        )
        .expect_err("runtime should fail with invalid readiness fd in this unit test");

        assert!(err
            .to_string()
            .contains("failed to signal net daemon readiness"));
    }
}
