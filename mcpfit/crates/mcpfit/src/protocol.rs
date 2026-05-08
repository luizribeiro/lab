use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[cfg(test)]
mod tests {
    use super::{
        ClientInfo, InitializeParams, InitializeResult, ServerCapabilities, ServerInfo,
        ToolsCapability,
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
