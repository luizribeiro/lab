use std::ffi::OsStr;

use fittings::{
    client::Client as FittingsClient, core::transport::Connector, SubprocessConnector,
};

use crate::error::McpfitError;
use crate::protocol::{ClientInfo, InitializeParams, InitializeResult};

const MCP_PROTOCOL_VERSION: &str = "2025-01-01";

pub struct Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    inner: FittingsClient<C>,
}

impl<C> Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    pub async fn connect(connector: C) -> Result<Self, McpfitError> {
        let client = Self::connect_uninitialized(connector).await?;
        client.initialize().await?;
        client.initialized().await?;
        Ok(client)
    }

    pub async fn connect_uninitialized(connector: C) -> Result<Self, McpfitError> {
        let inner = FittingsClient::connect(connector)
            .await
            .map_err(|e| McpfitError::internal(format!("fittings connect: {e}")))?;
        Ok(Self { inner })
    }

    pub async fn initialize(&self) -> Result<InitializeResult, McpfitError> {
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.into(),
            client_info: Some(ClientInfo {
                name: "mcpfit".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            }),
            capabilities: None,
        };
        let params_value = serde_json::to_value(&params)
            .map_err(|e| McpfitError::internal(format!("encode initialize params: {e}")))?;
        let result = self
            .inner
            .call("initialize", params_value)
            .await
            .map_err(|e| McpfitError::internal(format!("initialize call failed: {e}")))?;
        serde_json::from_value(result)
            .map_err(|e| McpfitError::internal(format!("decode initialize result: {e}")))
    }

    pub async fn initialized(&self) -> Result<(), McpfitError> {
        self.inner
            .notify("notifications/initialized", serde_json::json!({}))
            .await
            .map_err(|e| McpfitError::internal(format!("send initialized notification: {e}")))
    }
}

impl Client<SubprocessConnector> {
    pub async fn spawn(command: impl AsRef<OsStr>) -> Result<Self, McpfitError> {
        Self::connect(SubprocessConnector::new(command)).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fittings::{
        async_trait::async_trait,
        core::{error::FittingsError, transport::Connector, transport::Transport},
    };
    use fittings_testkit::{
        fixtures::{parse_request_fixture, success_response_line},
        memory_transport::MemoryTransport,
    };
    use serde_json::{json, Value};
    use tokio::sync::Mutex;

    use super::{Client, MCP_PROTOCOL_VERSION};
    use crate::protocol::{InitializeResult, ServerCapabilities, ServerInfo, ToolsCapability};

    struct OneShotConnector {
        transport: Arc<Mutex<Option<MemoryTransport>>>,
    }

    impl OneShotConnector {
        fn new(transport: MemoryTransport) -> Self {
            Self {
                transport: Arc::new(Mutex::new(Some(transport))),
            }
        }
    }

    #[async_trait]
    impl Connector for OneShotConnector {
        type Connection = MemoryTransport;

        async fn connect(&self) -> Result<Self::Connection, FittingsError> {
            self.transport
                .lock()
                .await
                .take()
                .ok_or_else(|| FittingsError::internal("connector already used"))
        }
    }

    #[tokio::test]
    async fn connect_uninitialized_does_not_send_any_handshake() {
        use std::time::Duration;

        use tokio::time::timeout;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);
        let _client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect without handshake");

        let waited = timeout(Duration::from_millis(50), server_transport.recv()).await;
        assert!(
            waited.is_err(),
            "connect_uninitialized must not write any MCP traffic, got: {:?}",
            waited.ok()
        );
    }

    #[tokio::test]
    async fn connect_uninitialized_surfaces_connector_failure() {
        struct FailingConnector;

        #[async_trait]
        impl Connector for FailingConnector {
            type Connection = MemoryTransport;

            async fn connect(&self) -> Result<Self::Connection, FittingsError> {
                Err(FittingsError::transport("simulated connect failure"))
            }
        }

        let err = match Client::connect_uninitialized(FailingConnector).await {
            Ok(_) => panic!("failing connector should propagate as McpfitError"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("simulated connect failure"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn initialize_sends_typed_request_and_decodes_result() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("initialize request");
            let request = parse_request_fixture(&frame).expect("decode request");
            assert_eq!(request.method, "initialize");
            let id = request.id.expect("initialize request must carry id");
            let params = request.params.expect("initialize request must carry params");
            assert_eq!(
                params.get("protocolVersion"),
                Some(&Value::String(MCP_PROTOCOL_VERSION.into()))
            );
            assert_eq!(
                params.get("clientInfo").and_then(|v| v.get("name")),
                Some(&Value::String("mcpfit".into()))
            );
            assert_eq!(
                params.get("clientInfo").and_then(|v| v.get("version")),
                Some(&Value::String(env!("CARGO_PKG_VERSION").into()))
            );

            let response = success_response_line(
                id,
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {"tools": {"listChanged": true}},
                    "serverInfo": {"name": "test-srv", "version": "9.9.9"},
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send initialize response");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let result = client.initialize().await.expect("initialize succeeds");

        assert_eq!(
            result,
            InitializeResult {
                protocol_version: MCP_PROTOCOL_VERSION.into(),
                capabilities: ServerCapabilities {
                    tools: Some(ToolsCapability {
                        list_changed: Some(true),
                    }),
                },
                server_info: ServerInfo {
                    name: "test-srv".into(),
                    version: "9.9.9".into(),
                },
            }
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn initialized_sends_notification_without_id() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        client
            .initialized()
            .await
            .expect("initialized notification succeeds");

        let frame = server_transport
            .recv()
            .await
            .expect("initialized notification frame");
        let request = parse_request_fixture(&frame).expect("decode notification");
        assert_eq!(request.method, "notifications/initialized");
        assert!(
            request.id.is_none(),
            "notifications/initialized must not carry an id, got: {:?}",
            request.id
        );
        assert_eq!(request.params, Some(json!({})));
    }

    #[tokio::test]
    async fn connect_runs_initialize_then_initialized_in_order() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let init_frame = server_transport.recv().await.expect("initialize request");
            let init_request = parse_request_fixture(&init_frame).expect("decode initialize");
            assert_eq!(init_request.method, "initialize");
            let id = init_request.id.expect("initialize must carry id");

            let response = success_response_line(
                id,
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {"tools": {"listChanged": true}},
                    "serverInfo": {"name": "test-srv", "version": "9.9.9"},
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send initialize response");

            let notif_frame = server_transport
                .recv()
                .await
                .expect("initialized notification");
            let notif = parse_request_fixture(&notif_frame).expect("decode notification");
            assert_eq!(notif.method, "notifications/initialized");
            assert!(
                notif.id.is_none(),
                "notifications/initialized must not carry an id, got: {:?}",
                notif.id
            );
        });

        let _client = Client::connect(OneShotConnector::new(client_transport))
            .await
            .expect("connect performs full handshake");

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn connect_surfaces_initialize_failure() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("initialize request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("request id");
            let response = success_response_line(id, json!({"not": "an initialize result"}))
                .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send malformed result");
        });

        let err = match Client::connect(OneShotConnector::new(client_transport)).await {
            Ok(_) => panic!("malformed initialize must abort connect"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("decode initialize result"),
            "unexpected error: {err}"
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn initialize_surfaces_decode_failure_when_result_is_malformed() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("initialize request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("request id");
            let response = success_response_line(id, json!({"not": "an initialize result"}))
                .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send malformed result");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let err = client
            .initialize()
            .await
            .expect_err("malformed result must surface as error");
        assert!(
            err.to_string().contains("decode initialize result"),
            "unexpected error: {err}"
        );

        server.await.expect("server task joins");
    }
}

#[cfg(all(test, unix))]
mod spawn_tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::Client;
    use super::MCP_PROTOCOL_VERSION;

    fn unique_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mcpfit-{name}-{}-{nanos}", std::process::id()))
    }

    fn write_executable_script(path: &Path, content: &str) {
        fs::write(path, content).expect("write script fixture");
        let mut perms = fs::metadata(path)
            .expect("read script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set executable permissions");
    }

    #[tokio::test]
    async fn spawn_runs_full_handshake_against_subprocess() {
        let script_path = unique_path("client-spawn");
        write_executable_script(
            &script_path,
            &format!(
                r#"#!/bin/sh
if [ "$FITTINGS" != "1" ]; then
  exit 90
fi
if [ "$1" != "serve" ]; then
  exit 91
fi
if [ -n "$2" ]; then
  exit 92
fi
IFS= read -r init_line || exit 1
id=$(printf '%s' "$init_line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
printf '{{"jsonrpc":"2.0","id":"%s","result":{{"protocolVersion":"{ver}","capabilities":{{"tools":{{"listChanged":true}}}},"serverInfo":{{"name":"spawn-srv","version":"0.0.0"}}}}}}\n' "$id"
IFS= read -r notif_line || exit 1
case "$notif_line" in
  *notifications/initialized*) ;;
  *) exit 93 ;;
esac
exec cat > /dev/null
"#,
                ver = MCP_PROTOCOL_VERSION,
            ),
        );

        let client = Client::spawn(&script_path)
            .await
            .expect("Client::spawn should perform full handshake");
        drop(client);

        let _ = fs::remove_file(script_path);
    }
}
