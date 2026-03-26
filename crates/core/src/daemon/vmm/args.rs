use anyhow::{bail, Context, Result};

use super::spec::VmmLaunchSpec;

pub const LAUNCH_SPEC_JSON_FLAG: &str = "--launch-spec-json";
pub const VMM_USAGE: &str = "usage: capsa-vmm --launch-spec-json <json>";

pub fn encode_launch_spec_args(spec: &VmmLaunchSpec) -> Result<Vec<String>> {
    let launch_spec_json =
        serde_json::to_string(spec).context("failed to serialize VMM launch spec")?;
    Ok(vec![LAUNCH_SPEC_JSON_FLAG.to_string(), launch_spec_json])
}

pub fn parse_launch_spec_args<I, S>(args: I) -> Result<VmmLaunchSpec>
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
        bail!(VMM_USAGE);
    }

    serde_json::from_str(
        launch_spec_json
            .as_deref()
            .expect("checked above: launch spec json is present"),
    )
    .context("failed to parse VMM launch spec JSON")
}

#[cfg(test)]
mod tests {
    use super::{parse_launch_spec_args, LAUNCH_SPEC_JSON_FLAG, VMM_USAGE};
    use crate::VmConfig;

    fn sample_launch_spec_json() -> String {
        serde_json::json!({
            "vm_config": {
                "root": "/tmp/root",
                "kernel": null,
                "initramfs": null,
                "kernel_cmdline": null,
                "vcpus": 1,
                "memory_mib": 512,
                "verbosity": 0,
                "interfaces": []
            },
            "resolved_interfaces": []
        })
        .to_string()
    }

    #[test]
    fn parse_launch_spec_args_accepts_valid_input() {
        let parsed =
            parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, &sample_launch_spec_json()])
                .expect("valid args should parse");

        assert_eq!(parsed.vm_config.vcpus, 1);
        assert!(parsed.resolved_interfaces.is_empty());
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
            assert_eq!(err.to_string(), VMM_USAGE);
        }
    }

    #[test]
    fn parse_launch_spec_args_reports_json_parse_errors() {
        let err = parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, "{not-json"])
            .expect_err("invalid json should fail");
        assert!(err
            .to_string()
            .contains("failed to parse VMM launch spec JSON"));
    }

    #[test]
    fn parse_output_type_is_vmm_launch_spec() {
        let parsed =
            parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, &sample_launch_spec_json()])
                .expect("valid args should parse");
        let _: VmConfig = parsed.vm_config;
    }
}
