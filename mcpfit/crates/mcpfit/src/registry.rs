use serde_json::json;

use crate::error::McpfitError;
use crate::protocol::ToolInfo;
use crate::tool::Tool;
use crate::Result;

#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Tool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Tool) -> Result<()> {
        if self.tools.iter().any(|t| t.name() == tool.name()) {
            return Err(McpfitError::invalid_request(format!(
                "tool already registered: {}",
                tool.name()
            )));
        }
        self.tools.push(tool);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name() == name)
    }

    pub fn list(&self) -> Vec<ToolInfo> {
        let mut infos: Vec<ToolInfo> = self
            .tools
            .iter()
            .map(|t| ToolInfo {
                name: t.name().to_string(),
                description: t.description_str().map(str::to_string),
                input_schema: t
                    .input_schema_value()
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"})),
            })
            .collect();
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }
}

#[cfg(test)]
mod tests {
    use super::ToolRegistry;
    use crate::error::McpfitError;
    use crate::tool::Tool;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(JsonSchema, Deserialize)]
    #[allow(dead_code)]
    struct AddArgs {
        a: f64,
        b: f64,
    }

    #[test]
    fn new_registry_is_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn register_appends_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Tool::new("a")).unwrap();
        registry.register(Tool::new("b")).unwrap();
        assert_eq!(registry.len(), 2);
        assert!(registry.contains("a"));
        assert!(registry.contains("b"));
    }

    #[test]
    fn register_rejects_duplicate_names() {
        let mut registry = ToolRegistry::new();
        registry.register(Tool::new("dup")).unwrap();
        let err = registry.register(Tool::new("dup")).unwrap_err();
        assert!(matches!(err, McpfitError::InvalidRequest(m) if m.contains("dup")));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn list_is_sorted_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register(Tool::new("charlie")).unwrap();
        registry.register(Tool::new("alpha")).unwrap();
        registry.register(Tool::new("bravo")).unwrap();
        let names: Vec<String> = registry.list().into_iter().map(|i| i.name).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn list_includes_description_and_schema() {
        let mut registry = ToolRegistry::new();
        registry
            .register(
                Tool::new("add")
                    .description("Adds two numbers")
                    .input::<AddArgs>(),
            )
            .unwrap();
        let info = &registry.list()[0];
        assert_eq!(info.name, "add");
        assert_eq!(info.description.as_deref(), Some("Adds two numbers"));
        let props = info.input_schema["properties"].as_object().unwrap();
        assert!(props.contains_key("a"));
    }

    #[test]
    fn list_defaults_missing_schema_to_empty_object() {
        let mut registry = ToolRegistry::new();
        registry.register(Tool::new("noschema")).unwrap();
        let info = &registry.list()[0];
        assert_eq!(info.description, None);
        assert_eq!(info.input_schema, json!({"type": "object"}));
    }
}
