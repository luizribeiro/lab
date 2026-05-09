use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCallParams {
    pub name: String,
    #[serde(default = "empty_object")]
    pub arguments: Value,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotificationParams {
    pub progress_token: Value,
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRegisterParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub response_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRegisterResult {
    pub tool: ToolInfo,
}

#[cfg(test)]
mod tests {
    use super::{
        ClientInfo, InitializeParams, InitializeResult, ProgressNotificationParams,
        ServerCapabilities, ServerInfo, ToolInfo, ToolsCallParams, ToolsCapability,
        ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
    };
    use serde_json::{json, to_value};

    #[test]
    fn initialize_params_serializes_in_camel_case_and_skips_none() {
        let encoded = to_value(InitializeParams {
            protocol_version: "2025-01-01".into(),
            client_info: Some(ClientInfo {
                name: "demo".into(),
                version: "0.1.0".into(),
            }),
            capabilities: None,
        })
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "protocolVersion": "2025-01-01",
                "clientInfo": {"name": "demo", "version": "0.1.0"},
            })
        );
    }

    #[test]
    fn initialize_params_deserializes_with_absent_optionals() {
        let decoded: InitializeParams =
            serde_json::from_value(json!({"protocolVersion": "2025-01-01"})).expect("deserialize");
        assert_eq!(
            decoded,
            InitializeParams {
                protocol_version: "2025-01-01".into(),
                client_info: None,
                capabilities: None,
            }
        );
    }

    #[test]
    fn initialize_result_serializes_with_typed_capabilities() {
        let encoded = to_value(InitializeResult {
            protocol_version: "2025-01-01".into(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
            },
            server_info: ServerInfo {
                name: "srv".into(),
                version: "1.2.3".into(),
            },
        })
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "protocolVersion": "2025-01-01",
                "capabilities": {"tools": {"listChanged": true}},
                "serverInfo": {"name": "srv", "version": "1.2.3"},
            })
        );
    }

    #[test]
    fn empty_server_capabilities_serializes_as_empty_object() {
        let encoded = to_value(ServerCapabilities::default()).expect("serialize");
        assert_eq!(encoded, json!({}));
    }

    #[test]
    fn tool_info_serializes_with_camel_case_and_skips_missing_description() {
        let encoded = to_value(ToolInfo {
            name: "add".into(),
            description: None,
            input_schema: json!({"type": "object"}),
        })
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "name": "add",
                "inputSchema": {"type": "object"},
            })
        );
    }

    #[test]
    fn tools_list_result_round_trips_through_json() {
        let original = ToolsListResult {
            tools: vec![ToolInfo {
                name: "echo".into(),
                description: Some("Echo the input.".into()),
                input_schema: json!({"type": "object"}),
            }],
        };
        let encoded = serde_json::to_string(&original).expect("serialize");
        let decoded: ToolsListResult = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn tools_call_params_defaults_arguments_to_empty_object() {
        let decoded: ToolsCallParams =
            serde_json::from_value(json!({"name": "ping"})).expect("deserialize");
        assert_eq!(
            decoded,
            ToolsCallParams {
                name: "ping".into(),
                arguments: json!({}),
                meta: None,
            }
        );
    }

    #[test]
    fn tools_call_params_round_trips_meta_under_underscore_meta() {
        let original = ToolsCallParams {
            name: "add".into(),
            arguments: json!({"a": 1, "b": 2}),
            meta: Some(json!({"progressToken": "tok"})),
        };
        let encoded = to_value(&original).expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "name": "add",
                "arguments": {"a": 1, "b": 2},
                "_meta": {"progressToken": "tok"},
            })
        );
        let decoded: ToolsCallParams = serde_json::from_value(encoded).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn tools_register_params_serializes_in_camel_case_and_skips_none() {
        let encoded = to_value(ToolsRegisterParams {
            name: "ping".into(),
            description: None,
            response_text: "pong".into(),
        })
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "name": "ping",
                "responseText": "pong",
            })
        );
    }

    #[test]
    fn tools_register_result_round_trips_through_json() {
        let original = ToolsRegisterResult {
            tool: ToolInfo {
                name: "ping".into(),
                description: Some("Static responder.".into()),
                input_schema: json!({"type": "object"}),
            },
        };
        let encoded = serde_json::to_string(&original).expect("serialize");
        let decoded: ToolsRegisterResult = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, original);
    }

    #[test]
    fn progress_notification_params_deserializes_full_payload() {
        let decoded: ProgressNotificationParams = serde_json::from_value(json!({
            "progressToken": "tok-1",
            "progress": 0.5,
            "total": 1.0,
            "message": "halfway",
        }))
        .expect("deserialize");
        assert_eq!(decoded.progress_token, json!("tok-1"));
        assert_eq!(decoded.progress, 0.5);
        assert_eq!(decoded.total, Some(1.0));
        assert_eq!(decoded.message.as_deref(), Some("halfway"));
    }

    #[test]
    fn progress_notification_params_deserializes_with_integer_token_and_no_optionals() {
        let decoded: ProgressNotificationParams = serde_json::from_value(json!({
            "progressToken": 7,
            "progress": 3.0,
        }))
        .expect("deserialize");
        assert_eq!(decoded.progress_token, json!(7));
        assert_eq!(decoded.progress, 3.0);
        assert_eq!(decoded.total, None);
        assert_eq!(decoded.message, None);
    }

    #[test]
    fn progress_notification_params_skips_none_optionals_on_serialize() {
        let encoded = to_value(ProgressNotificationParams {
            progress_token: json!("tok-2"),
            progress: 1.0,
            total: None,
            message: None,
        })
        .expect("serialize");
        assert_eq!(
            encoded,
            json!({
                "progressToken": "tok-2",
                "progress": 1.0,
            })
        );
    }

    #[test]
    fn initialize_result_round_trips_through_json() {
        let original = InitializeResult {
            protocol_version: "2025-01-01".into(),
            capabilities: ServerCapabilities::default(),
            server_info: ServerInfo {
                name: "srv".into(),
                version: "0.1.0".into(),
            },
        };
        let encoded = serde_json::to_string(&original).expect("serialize");
        let decoded: InitializeResult = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, original);
    }
}
