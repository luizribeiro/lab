use serde::Serialize;

use crate::content::ToolContent;
use crate::response::{IntoToolResponse, ToolResponse};

/// Marker trait for types usable as structured tool output.
///
/// The trait carries no methods: it exists purely so the type system can
/// require structured-content payloads to be objects at the source level. It
/// does not validate the runtime JSON shape beyond what `schemars` produces.
pub trait StructuredObject {}

/// Wrapper that carries a [`StructuredObject`] value plus an optional text
/// override. When no override is supplied, response conversion renders the
/// text block as compact JSON; [`Structured::with_text`] supplies a custom
/// human-readable form instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structured<T> {
    value: T,
    text: Option<String>,
}

impl<T> Structured<T>
where
    T: StructuredObject,
{
    pub fn new(value: T) -> Self {
        Self { value, text: None }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn text_override(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn into_parts(self) -> (T, Option<String>) {
        (self.value, self.text)
    }
}

impl<T> IntoToolResponse for Structured<T>
where
    T: Serialize + StructuredObject,
{
    fn into_tool_response(self) -> ToolResponse {
        let (value, text) = self.into_parts();
        let structured = serde_json::to_value(&value)
            .expect("StructuredObject value must serialize to JSON");
        let text = text.unwrap_or_else(|| structured.to_string());
        ToolResponse {
            content: vec![ToolContent::text(text)],
            structured_content: Some(structured),
            is_error: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IntoToolResponse, Structured, StructuredObject, ToolContent, ToolResponse};
    use serde::Serialize;
    use serde_json::json;

    #[derive(Debug, PartialEq, Eq, Serialize)]
    struct Sum {
        total: i64,
    }

    impl StructuredObject for Sum {}

    fn assert_structured<T: StructuredObject>() {}

    #[test]
    fn manual_impl_satisfies_marker() {
        assert_structured::<Sum>();
    }

    #[test]
    fn new_stores_value_without_text_override() {
        let s = Structured::new(Sum { total: 5 });
        assert_eq!(s.value(), &Sum { total: 5 });
        assert_eq!(s.text_override(), None);
    }

    #[test]
    fn with_text_sets_override() {
        let s = Structured::new(Sum { total: 7 }).with_text("seven");
        assert_eq!(s.text_override(), Some("seven"));
    }

    #[test]
    fn into_parts_returns_value_and_text() {
        let (value, text) = Structured::new(Sum { total: 3 })
            .with_text("three")
            .into_parts();
        assert_eq!(value, Sum { total: 3 });
        assert_eq!(text.as_deref(), Some("three"));
    }

    #[test]
    fn into_tool_response_uses_compact_json_default_text() {
        let response = Structured::new(Sum { total: 5 }).into_tool_response();
        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text(r#"{"total":5}"#)],
                structured_content: Some(json!({"total": 5})),
                is_error: false,
            }
        );
    }

    #[test]
    fn into_tool_response_honors_text_override() {
        let response = Structured::new(Sum { total: 9 })
            .with_text("nine")
            .into_tool_response();
        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text("nine")],
                structured_content: Some(json!({"total": 9})),
                is_error: false,
            }
        );
    }

    #[test]
    fn into_tool_response_is_never_error() {
        let response = Structured::new(Sum { total: 1 }).into_tool_response();
        assert!(!response.is_error);
    }
}
