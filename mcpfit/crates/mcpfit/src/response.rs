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

pub trait IntoToolResponse {
    fn into_tool_response(self) -> ToolResponse;
}

impl IntoToolResponse for ToolResponse {
    fn into_tool_response(self) -> ToolResponse {
        self
    }
}

impl IntoToolResponse for String {
    fn into_tool_response(self) -> ToolResponse {
        ToolResponse::success(self)
    }
}

impl IntoToolResponse for &'static str {
    fn into_tool_response(self) -> ToolResponse {
        ToolResponse::success(self)
    }
}

macro_rules! impl_into_tool_response_via_display {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoToolResponse for $ty {
                fn into_tool_response(self) -> ToolResponse {
                    ToolResponse::success(self.to_string())
                }
            }
        )*
    };
}

impl_into_tool_response_via_display!(
    bool, f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize,
);

#[cfg(test)]
mod tests {
    use super::{IntoToolResponse, ToolResponse};
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
    fn string_converts_to_text_response() {
        let response = String::from("hello").into_tool_response();
        assert_eq!(response, ToolResponse::success("hello"));
    }

    #[test]
    fn static_str_converts_to_text_response() {
        let response = "hello".into_tool_response();
        assert_eq!(response, ToolResponse::success("hello"));
    }

    #[test]
    fn numeric_primitives_render_via_display() {
        assert_eq!(42_i64.into_tool_response(), ToolResponse::success("42"));
        assert_eq!(7_u8.into_tool_response(), ToolResponse::success("7"));
        assert_eq!(1.5_f64.into_tool_response(), ToolResponse::success("1.5"));
    }

    #[test]
    fn bool_renders_via_display() {
        assert_eq!(true.into_tool_response(), ToolResponse::success("true"));
        assert_eq!(false.into_tool_response(), ToolResponse::success("false"));
    }

    #[test]
    fn tool_response_passthrough_preserves_success() {
        let original = ToolResponse::success("hi").with_structured(json!({"n": 1}));
        assert_eq!(original.clone().into_tool_response(), original);
    }

    #[test]
    fn tool_response_passthrough_preserves_error_flag() {
        let original = ToolResponse::error("boom");
        let converted = original.clone().into_tool_response();
        assert!(converted.is_error);
        assert_eq!(converted, original);
    }

    #[test]
    fn tool_response_passthrough_preserves_multi_content() {
        let original = ToolResponse::success("primary")
            .with_content(vec![ToolContent::text("a"), ToolContent::text("b")]);
        assert_eq!(original.clone().into_tool_response(), original);
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
