//! Builder for tool definitions.

use schemars::JsonSchema;
use serde_json::Value;

use crate::schema::schema_for;

/// Builder for a single MCP tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    name: String,
    description: Option<String>,
    input_schema: Option<Value>,
}

impl Tool {
    /// Starts a new tool builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: None,
        }
    }

    /// Sets the human-readable description advertised to clients.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn input<T: JsonSchema>(mut self) -> Self {
        self.input_schema = Some(schema_for::<T>());
        self
    }

    /// Mutually exclusive with [`Tool::input`] and [`Tool::input_with_schema`];
    /// the most recent call wins.
    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Reserves `T` as the Rust type used for argument deserialization once
    /// handlers are wired up, while advertising the supplied hand-tuned schema.
    pub fn input_with_schema<T>(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description_str(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn input_schema_value(&self) -> Option<&Value> {
        self.input_schema.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::Tool;
    use schemars::JsonSchema;

    #[derive(JsonSchema)]
    #[allow(dead_code)]
    struct AddArgs {
        a: f64,
        b: f64,
    }

    #[test]
    fn new_stores_name_without_description() {
        let tool = Tool::new("add");
        assert_eq!(tool.name(), "add");
        assert_eq!(tool.description_str(), None);
        assert!(tool.input_schema_value().is_none());
    }

    #[test]
    fn description_sets_value() {
        let tool = Tool::new("add").description("Adds two numbers");
        assert_eq!(tool.description_str(), Some("Adds two numbers"));
    }

    #[test]
    fn description_overrides_previous_value() {
        let tool = Tool::new("add")
            .description("first")
            .description("second");
        assert_eq!(tool.description_str(), Some("second"));
    }

    #[test]
    fn name_accepts_string_and_str() {
        let from_str = Tool::new("a");
        let from_string = Tool::new(String::from("a"));
        assert_eq!(from_str, from_string);
    }

    #[test]
    fn input_overrides_previous_schema() {
        #[derive(JsonSchema)]
        #[allow(dead_code)]
        struct Other {
            x: i32,
        }

        let tool = Tool::new("t").input::<AddArgs>().input::<Other>();
        let props = tool.input_schema_value().unwrap()["properties"]
            .as_object()
            .unwrap();
        assert!(props.contains_key("x"));
        assert!(!props.contains_key("a"));
    }

    #[test]
    fn input_schema_sets_hand_written_schema() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": { "a": { "type": "number", "description": "first" } },
            "required": ["a"],
        });
        let tool = Tool::new("t").input_schema(raw.clone());
        assert_eq!(tool.input_schema_value(), Some(&raw));
    }

    #[test]
    fn input_schema_overrides_previous_typed_schema() {
        let raw = serde_json::json!({"type": "object"});
        let tool = Tool::new("t").input::<AddArgs>().input_schema(raw.clone());
        assert_eq!(tool.input_schema_value(), Some(&raw));
    }

    #[test]
    fn input_with_schema_stores_hand_tuned_schema() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": { "a": { "type": "number", "description": "tuned" } },
        });
        let tool = Tool::new("t").input_with_schema::<AddArgs>(raw.clone());
        assert_eq!(tool.input_schema_value(), Some(&raw));
    }

    #[test]
    fn typed_input_overrides_hand_written_schema() {
        let raw = serde_json::json!({"type": "object"});
        let tool = Tool::new("t").input_schema(raw).input::<AddArgs>();
        let props = tool.input_schema_value().unwrap()["properties"]
            .as_object()
            .unwrap();
        assert!(props.contains_key("a"));
    }

    #[test]
    fn input_preserves_name_and_description() {
        let tool = Tool::new("add")
            .description("Adds two numbers")
            .input::<AddArgs>();
        assert_eq!(tool.name(), "add");
        assert_eq!(tool.description_str(), Some("Adds two numbers"));
        assert!(tool.input_schema_value().is_some());
    }
}
