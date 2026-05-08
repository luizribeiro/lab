//! Internal JSON Schema generation helpers.

use schemars::JsonSchema;
use serde_json::Value;

/// Generates a JSON Schema for `T` and serializes it as a [`serde_json::Value`].
///
/// Used by the tool builder to derive `inputSchema` from typed argument
/// definitions. Kept crate-private so the public surface only exposes the
/// builder methods that consume it.
pub(crate) fn schema_for<T: JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);
    serde_json::to_value(&schema).expect("schemars output must serialize as JSON")
}

#[cfg(test)]
mod tests {
    use super::schema_for;
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(Deserialize, JsonSchema)]
    #[allow(dead_code)]
    struct AddArgs {
        a: f64,
        b: f64,
    }

    #[test]
    fn schema_describes_object_with_named_properties() {
        let schema = schema_for::<AddArgs>();
        assert_eq!(schema["type"], "object");
        let props = schema["properties"].as_object().expect("object props");
        assert!(props.contains_key("a"));
        assert!(props.contains_key("b"));
    }

    #[test]
    fn schema_includes_title_for_named_struct() {
        let schema = schema_for::<AddArgs>();
        assert_eq!(schema["title"], "AddArgs");
    }

    #[test]
    fn schema_for_unit_is_serializable() {
        let schema = schema_for::<()>();
        assert!(schema.is_object());
    }
}
