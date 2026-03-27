use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceSchema {
    pub name: String,
    #[serde(default)]
    pub methods: Vec<MethodSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodSchema {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchemaValidationError {
    #[error("service schema `name` is required")]
    MissingServiceName,
    #[error("method name is required")]
    MissingMethodName,
    #[error("duplicate method name `{0}`")]
    DuplicateMethodName(String),
}

pub fn validate_service_schema(schema: &ServiceSchema) -> Result<(), SchemaValidationError> {
    if schema.name.trim().is_empty() {
        return Err(SchemaValidationError::MissingServiceName);
    }

    let mut seen = HashSet::new();
    for method in &schema.methods {
        if method.name.trim().is_empty() {
            return Err(SchemaValidationError::MissingMethodName);
        }

        if !seen.insert(method.name.as_str()) {
            return Err(SchemaValidationError::DuplicateMethodName(
                method.name.clone(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_service_schema, MethodSchema, SchemaValidationError, ServiceSchema};

    #[test]
    fn schema_validation_rejects_missing_required_fields() {
        let missing_service_name = ServiceSchema {
            name: "  ".to_string(),
            methods: vec![],
            config_schema: None,
        };

        assert!(matches!(
            validate_service_schema(&missing_service_name),
            Err(SchemaValidationError::MissingServiceName)
        ));

        let missing_method_name = ServiceSchema {
            name: "hello-service".to_string(),
            methods: vec![MethodSchema {
                name: "".to_string(),
                description: None,
                params_schema: None,
                result_schema: None,
            }],
            config_schema: None,
        };

        assert!(matches!(
            validate_service_schema(&missing_method_name),
            Err(SchemaValidationError::MissingMethodName)
        ));
    }

    #[test]
    fn schema_validation_rejects_duplicate_method_names() {
        let schema = ServiceSchema {
            name: "hello-service".to_string(),
            methods: vec![
                MethodSchema {
                    name: "hello".to_string(),
                    description: None,
                    params_schema: None,
                    result_schema: None,
                },
                MethodSchema {
                    name: "hello".to_string(),
                    description: None,
                    params_schema: None,
                    result_schema: None,
                },
            ],
            config_schema: None,
        };

        assert!(matches!(
            validate_service_schema(&schema),
            Err(SchemaValidationError::DuplicateMethodName(name)) if name == "hello"
        ));
    }

    #[test]
    fn schema_validation_accepts_unique_method_names() {
        let schema = ServiceSchema {
            name: "hello-service".to_string(),
            methods: vec![
                MethodSchema {
                    name: "hello".to_string(),
                    description: None,
                    params_schema: None,
                    result_schema: None,
                },
                MethodSchema {
                    name: "ping".to_string(),
                    description: None,
                    params_schema: None,
                    result_schema: None,
                },
            ],
            config_schema: None,
        };

        validate_service_schema(&schema).expect("schema should be valid");
    }
}
