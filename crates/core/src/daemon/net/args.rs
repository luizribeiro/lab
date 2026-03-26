use anyhow::{bail, Context, Result};

use super::spec::NetLaunchSpec;

pub const LAUNCH_SPEC_JSON_FLAG: &str = "--launch-spec-json";
pub const NETD_USAGE: &str = "usage: capsa-netd --launch-spec-json <json>";

pub fn encode_launch_spec_args(spec: &NetLaunchSpec) -> Result<Vec<String>> {
    let launch_spec_json =
        serde_json::to_string(spec).context("failed to serialize net daemon launch spec")?;
    Ok(vec![LAUNCH_SPEC_JSON_FLAG.to_string(), launch_spec_json])
}

pub fn parse_launch_spec_args<I, S>(args: I) -> Result<NetLaunchSpec>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);

    let flag = args.next();
    let launch_spec_json = args.next();

    if flag.as_deref() != Some(LAUNCH_SPEC_JSON_FLAG)
        || launch_spec_json.is_none()
        || args.next().is_some()
    {
        bail!(NETD_USAGE);
    }

    serde_json::from_str(
        launch_spec_json
            .as_deref()
            .expect("checked above: launch spec json is present"),
    )
    .context("failed to parse net daemon launch spec JSON")
}

#[cfg(test)]
mod tests {
    use super::{
        encode_launch_spec_args, parse_launch_spec_args, LAUNCH_SPEC_JSON_FLAG, NETD_USAGE,
    };
    use crate::daemon::net::spec::NetLaunchSpec;

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

    #[test]
    fn parse_launch_spec_args_accepts_valid_input() {
        let parsed =
            parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, &sample_launch_spec_json()])
                .expect("valid args should parse");

        assert_eq!(parsed.interfaces.len(), 1);
        assert_eq!(parsed.interfaces[0].host_fd, 200);
    }

    #[test]
    fn parse_launch_spec_args_rejects_usage_errors() {
        for args in [
            vec![],
            vec!["--wrong-flag"],
            vec![LAUNCH_SPEC_JSON_FLAG],
            vec![LAUNCH_SPEC_JSON_FLAG, "{}", "extra"],
        ] {
            let err = parse_launch_spec_args(args).expect_err("usage errors should fail");
            assert_eq!(err.to_string(), NETD_USAGE);
        }
    }

    #[test]
    fn parse_launch_spec_args_reports_json_parse_errors() {
        let err = parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, "{not-json"])
            .expect_err("invalid json should fail");
        assert!(err
            .to_string()
            .contains("failed to parse net daemon launch spec JSON"));
    }

    #[test]
    fn encode_and_parse_round_trip() {
        let expected = NetLaunchSpec { interfaces: vec![] };

        let encoded = encode_launch_spec_args(&expected).expect("encoding should succeed");
        assert_eq!(encoded[0], LAUNCH_SPEC_JSON_FLAG);

        let decoded = parse_launch_spec_args(encoded).expect("round-trip parse should succeed");
        assert_eq!(decoded, expected);
    }
}
