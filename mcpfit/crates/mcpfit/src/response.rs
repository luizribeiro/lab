use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::content::ToolContent;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponse {
    pub content: Vec<ToolContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResponse {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            structured_content: None,
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            structured_content: None,
            is_error: true,
        }
    }

    pub fn with_content(mut self, content: Vec<ToolContent>) -> Self {
        self.content = content;
        self
    }

    pub fn with_structured(mut self, structured_content: Value) -> Self {
        self.structured_content = Some(structured_content);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::ToolResponse;
    use crate::content::ToolContent;
    use serde_json::{json, to_value};

    #[test]
    fn success_serializes_with_text_content_and_no_error() {
        let encoded = to_value(ToolResponse::success("ok")).expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "content": [{"type": "text", "text": "ok"}],
                "isError": false,
            })
        );
    }

    #[test]
    fn error_sets_is_error_true_with_text_content() {
        let encoded = to_value(ToolResponse::error("boom")).expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "content": [{"type": "text", "text": "boom"}],
                "isError": true,
            })
        );
    }

    #[test]
    fn structured_content_serializes_in_camel_case() {
        let encoded = to_value(
            ToolResponse::success("sum=5").with_structured(json!({"sum": 5})),
        )
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "content": [{"type": "text", "text": "sum=5"}],
                "structuredContent": {"sum": 5},
                "isError": false,
            })
        );
    }

    #[test]
    fn round_trips_through_json() {
        let original = ToolResponse {
            content: vec![ToolContent::text("hi")],
            structured_content: Some(json!({"k": "v"})),
            is_error: true,
        };
        let encoded = serde_json::to_string(&original).expect("serialize");
        let decoded: ToolResponse = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, original);
    }
}
