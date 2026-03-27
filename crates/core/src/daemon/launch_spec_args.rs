use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub const LAUNCH_SPEC_JSON_FLAG: &str = "--launch-spec-json";
const USAGE: &str = "usage: --launch-spec-json <json>";

pub fn encode_launch_spec_args<T: Serialize>(spec: &T) -> Result<Vec<String>> {
    let launch_spec_json =
        serde_json::to_string(spec).context("failed to serialize launch spec")?;
    Ok(vec![LAUNCH_SPEC_JSON_FLAG.to_string(), launch_spec_json])
}

pub fn parse_launch_spec_args<T, I, S>(args: I) -> Result<T>
where
    T: DeserializeOwned,
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
        bail!(USAGE);
    }

    serde_json::from_str(
        launch_spec_json
            .as_deref()
            .expect("checked above: launch spec json is present"),
    )
    .context("failed to parse launch spec JSON")
}

#[cfg(test)]
mod tests {
    use super::{encode_launch_spec_args, parse_launch_spec_args, LAUNCH_SPEC_JSON_FLAG};

    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct Spec {
        answer: u32,
    }

    #[test]
    fn parse_accepts_valid_input() {
        let parsed: Spec = parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, "{\"answer\":42}"])
            .expect("valid args should parse");
        assert_eq!(parsed.answer, 42);
    }

    #[test]
    fn parse_rejects_usage_errors() {
        for args in [
            vec![],
            vec!["--wrong-flag"],
            vec![LAUNCH_SPEC_JSON_FLAG],
            vec![LAUNCH_SPEC_JSON_FLAG, "{}", "extra"],
        ] {
            let err =
                parse_launch_spec_args::<Spec, _, _>(args).expect_err("usage errors should fail");
            assert_eq!(err.to_string(), "usage: --launch-spec-json <json>");
        }
    }

    #[test]
    fn encode_and_parse_round_trip() {
        let expected = Spec { answer: 7 };
        let encoded = encode_launch_spec_args(&expected).expect("encoding should succeed");
        let decoded: Spec =
            parse_launch_spec_args(encoded).expect("round-trip parse should succeed");
        assert_eq!(decoded, expected);
    }
}
