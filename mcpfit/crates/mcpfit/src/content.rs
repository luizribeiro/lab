use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum ToolContent {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        resource: EmbeddedResource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResource {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

impl ToolContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    pub fn resource(resource: EmbeddedResource) -> Self {
        Self::Resource { resource }
    }
}

#[cfg(test)]
mod tests {
    use super::{EmbeddedResource, ToolContent};
    use serde_json::{json, to_value};

    #[test]
    fn text_serializes_with_type_tag() {
        let encoded = to_value(ToolContent::text("hello")).expect("serialize");
        assert_eq!(encoded, json!({"type": "text", "text": "hello"}));
    }

    #[test]
    fn image_serializes_with_camelcase_mime_type() {
        let encoded = to_value(ToolContent::image("AAA=", "image/png")).expect("serialize");
        assert_eq!(
            encoded,
            json!({"type": "image", "data": "AAA=", "mimeType": "image/png"})
        );
    }

    #[test]
    fn resource_serializes_with_nested_object_and_skips_none() {
        let encoded = to_value(ToolContent::resource(EmbeddedResource {
            uri: "file:///a.txt".into(),
            mime_type: Some("text/plain".into()),
            text: Some("hi".into()),
            blob: None,
        }))
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "type": "resource",
                "resource": {
                    "uri": "file:///a.txt",
                    "mimeType": "text/plain",
                    "text": "hi"
                }
            })
        );
    }

    #[test]
    fn all_variants_round_trip() {
        for original in [
            ToolContent::text("hi"),
            ToolContent::image("AAA=", "image/png"),
            ToolContent::resource(EmbeddedResource {
                uri: "file:///a".into(),
                mime_type: Some("text/plain".into()),
                text: Some("hi".into()),
                blob: None,
            }),
        ] {
            let encoded = serde_json::to_string(&original).expect("serialize");
            let decoded: ToolContent = serde_json::from_str(&encoded).expect("deserialize");
            assert_eq!(decoded, original);
        }
    }

    #[test]
    fn resource_deserializes_with_absent_optional_fields() {
        let decoded: ToolContent =
            serde_json::from_value(json!({"type": "resource", "resource": {"uri": "file:///a"}}))
                .expect("deserialize");
        assert_eq!(
            decoded,
            ToolContent::resource(EmbeddedResource {
                uri: "file:///a".into(),
                mime_type: None,
                text: None,
                blob: None,
            })
        );
    }
}
