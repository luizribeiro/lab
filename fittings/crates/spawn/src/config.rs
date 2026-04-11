use serde_json::Value;
use thiserror::Error;

use crate::schema::ServiceSchema;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    #[error("invalid config JSON: {0}")]
    InvalidJson(String),
    #[error("server config must be a JSON object")]
    NotAnObject,
}

pub fn parse_server_config(
    raw: Option<&str>,
    _schema: &ServiceSchema,
) -> Result<Option<Value>, ConfigError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let parsed: Value =
        serde_json::from_str(raw).map_err(|error| ConfigError::InvalidJson(error.to_string()))?;

    if !parsed.is_object() {
        return Err(ConfigError::NotAnObject);
    }

    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::schema::ServiceSchema;

    use super::{parse_server_config, ConfigError};

    fn schema() -> ServiceSchema {
        ServiceSchema {
            name: "svc".to_string(),
            methods: vec![],
            config_schema: None,
        }
    }

    #[test]
    fn parse_server_config_accepts_none_and_object_json() {
        assert_eq!(
            parse_server_config(None, &schema()).expect("none should pass"),
            None
        );

        let parsed = parse_server_config(Some("{\"log_level\":\"debug\"}"), &schema())
            .expect("object config should parse");
        assert_eq!(parsed, Some(json!({"log_level": "debug"})));
    }

    #[test]
    fn parse_server_config_rejects_invalid_json_and_non_object_values() {
        let invalid_json = parse_server_config(Some("{"), &schema()).expect_err("must fail");
        assert!(matches!(invalid_json, ConfigError::InvalidJson(_)));

        let not_object = parse_server_config(Some("[1,2,3]"), &schema()).expect_err("must fail");
        assert_eq!(not_object, ConfigError::NotAnObject);
    }
}
