//! Builder for tool definitions.

/// Builder for a single MCP tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    name: String,
    description: Option<String>,
}

impl Tool {
    /// Starts a new tool builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    /// Sets the human-readable description advertised to clients.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description_str(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::Tool;

    #[test]
    fn new_stores_name_without_description() {
        let tool = Tool::new("add");
        assert_eq!(tool.name(), "add");
        assert_eq!(tool.description_str(), None);
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
}
