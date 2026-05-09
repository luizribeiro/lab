use std::collections::HashMap;
use std::ffi::OsStr;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use fittings::{
    client::{Client as FittingsClient, InboundNotification, PendingCall},
    core::transport::Connector,
    tokio::{
        sync::{broadcast, mpsc},
        task::JoinHandle,
    },
    SubprocessConnector,
};

use serde::Serialize;
use serde_json::Value;

use crate::error::McpfitError;
use crate::protocol::{
    ClientInfo, InitializeParams, InitializeResult, ProgressNotificationParams, ToolInfo,
    ToolsCallParams, ToolsListResult,
};
use crate::response::ToolResponse;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ProgressToken {
    String(String),
    Number(i64),
}

#[allow(dead_code)]
impl ProgressToken {
    pub(crate) fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(Self::String(s.clone())),
            Value::Number(n) => n.as_i64().map(Self::Number),
            _ => None,
        }
    }
}

pub(crate) type ProgressSender = mpsc::Sender<ProgressNotificationParams>;

pub(crate) const PROGRESS_CHANNEL_CAPACITY: usize = 64;

#[derive(Debug, Default)]
struct ProgressRegistryInner {
    senders: HashMap<ProgressToken, ProgressSender>,
    missed: HashMap<ProgressToken, Arc<AtomicU64>>,
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub(crate) struct ProgressRegistry {
    inner: Arc<Mutex<ProgressRegistryInner>>,
}

#[allow(dead_code)]
impl ProgressRegistry {
    fn lock(&self) -> std::sync::MutexGuard<'_, ProgressRegistryInner> {
        self.inner.lock().expect("progress registry lock poisoned")
    }

    pub(crate) fn register(
        &self,
        token: ProgressToken,
        sender: ProgressSender,
    ) -> Arc<AtomicU64> {
        let missed = Arc::new(AtomicU64::new(0));
        let mut inner = self.lock();
        inner.senders.insert(token.clone(), sender);
        inner.missed.insert(token, missed.clone());
        missed
    }

    pub(crate) fn remove(&self, token: &ProgressToken) -> Option<ProgressSender> {
        let mut inner = self.lock();
        inner.missed.remove(token);
        inner.senders.remove(token)
    }

    pub(crate) fn get(&self, token: &ProgressToken) -> Option<ProgressSender> {
        self.lock().senders.get(token).cloned()
    }

    pub(crate) fn missed_counter(&self, token: &ProgressToken) -> Option<Arc<AtomicU64>> {
        self.lock().missed.get(token).cloned()
    }

    pub(crate) fn close_all(&self) {
        let mut inner = self.lock();
        inner.senders.clear();
        inner.missed.clear();
    }

    fn deliver(&self, token: &ProgressToken, params: ProgressNotificationParams) {
        let inner = self.lock();
        let Some(sender) = inner.senders.get(token) else {
            return;
        };
        match sender.try_send(params) {
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                if let Some(missed) = inner.missed.get(token) {
                    missed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

const MCP_PROTOCOL_VERSION: &str = "2025-01-01";

pub struct Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    inner: FittingsClient<C>,
    router: Option<JoinHandle<()>>,
    progress: ProgressRegistry,
    next_progress_token: AtomicU64,
}

impl<C> Drop for Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    fn drop(&mut self) {
        if let Some(handle) = self.router.take() {
            handle.abort();
        }
        self.progress.close_all();
    }
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
        let progress = ProgressRegistry::default();
        let router = spawn_notification_router(inner.subscribe_notifications(), progress.clone());
        Ok(Self {
            inner,
            router: Some(router),
            progress,
            next_progress_token: AtomicU64::new(0),
        })
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

    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>, McpfitError> {
        let result = self
            .inner
            .call("tools/list", serde_json::json!({}))
            .await
            .map_err(|e| McpfitError::internal(format!("tools/list call failed: {e}")))?;
        let decoded: ToolsListResult = serde_json::from_value(result)
            .map_err(|e| McpfitError::internal(format!("decode tools/list result: {e}")))?;
        Ok(decoded.tools)
    }

    pub async fn call_tool(
        &self,
        name: impl Into<String>,
        args: impl Serialize,
    ) -> Result<ToolResponse, McpfitError> {
        let response = self.call_tool_raw(name, args).await?;
        if response.is_error {
            return Err(McpfitError::ToolFailed(response));
        }
        Ok(response)
    }

    pub async fn call_tool_raw(
        &self,
        name: impl Into<String>,
        args: impl Serialize,
    ) -> Result<ToolResponse, McpfitError> {
        let arguments = serde_json::to_value(args)
            .map_err(|e| McpfitError::internal(format!("encode tools/call arguments: {e}")))?;
        let params = ToolsCallParams {
            name: name.into(),
            arguments,
            meta: None,
        };
        let params_value = serde_json::to_value(&params)
            .map_err(|e| McpfitError::internal(format!("encode tools/call params: {e}")))?;
        let result = self
            .inner
            .call("tools/call", params_value)
            .await
            .map_err(|e| McpfitError::internal(format!("tools/call call failed: {e}")))?;
        serde_json::from_value(result)
            .map_err(|e| McpfitError::internal(format!("decode tools/call result: {e}")))
    }

    pub fn call_tool_with_progress<A>(
        &self,
        name: impl Into<String>,
        args: A,
    ) -> ProgressCallBuilder<'_, C, A>
    where
        A: Serialize,
    {
        ProgressCallBuilder {
            client: self,
            name: name.into(),
            args,
        }
    }

    pub fn notifications(&self) -> broadcast::Receiver<InboundNotification> {
        self.inner.subscribe_notifications()
    }

    #[allow(dead_code)]
    pub(crate) fn progress_registry(&self) -> &ProgressRegistry {
        &self.progress
    }

    pub async fn initialized(&self) -> Result<(), McpfitError> {
        self.inner
            .notify("notifications/initialized", serde_json::json!({}))
            .await
            .map_err(|e| McpfitError::internal(format!("send initialized notification: {e}")))
    }
}

pub(crate) struct HandleCleanup {
    registry: ProgressRegistry,
    token: ProgressToken,
}

impl Drop for HandleCleanup {
    fn drop(&mut self) {
        self.registry.remove(&self.token);
    }
}

pub struct ToolCallHandle {
    pending: PendingCall,
    progress_rx: mpsc::Receiver<ProgressNotificationParams>,
    missed: Arc<AtomicU64>,
    _cleanup: Option<HandleCleanup>,
}

impl ToolCallHandle {
    pub(crate) fn new(
        pending: PendingCall,
        progress_rx: mpsc::Receiver<ProgressNotificationParams>,
        missed: Arc<AtomicU64>,
        cleanup: Option<HandleCleanup>,
    ) -> Self {
        Self {
            pending,
            progress_rx,
            missed,
            _cleanup: cleanup,
        }
    }

    pub fn progress(&mut self) -> &mut mpsc::Receiver<ProgressNotificationParams> {
        &mut self.progress_rx
    }

    pub fn missed_progress_count(&self) -> u64 {
        self.missed.load(Ordering::Relaxed)
    }
}

impl IntoFuture for ToolCallHandle {
    type Output = Result<ToolResponse, McpfitError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let ToolCallHandle {
                pending, _cleanup, ..
            } = self;
            let value = pending
                .await
                .map_err(|e| McpfitError::internal(format!("tools/call call failed: {e}")))?;
            let response: ToolResponse = serde_json::from_value(value)
                .map_err(|e| McpfitError::internal(format!("decode tools/call result: {e}")))?;
            if response.is_error {
                return Err(McpfitError::ToolFailed(response));
            }
            Ok(response)
        })
    }
}

pub struct ProgressCallBuilder<'a, C, A>
where
    C: Connector + Send + Sync + 'static,
{
    client: &'a Client<C>,
    name: String,
    args: A,
}

impl<'a, C, A> ProgressCallBuilder<'a, C, A>
where
    C: Connector + Send + Sync + 'static,
    A: Serialize,
{
    pub async fn start(self) -> Result<ToolCallHandle, McpfitError> {
        let arguments = serde_json::to_value(self.args)
            .map_err(|e| McpfitError::internal(format!("encode tools/call arguments: {e}")))?;
        let token_n = self
            .client
            .next_progress_token
            .fetch_add(1, Ordering::Relaxed);
        let token_str = format!("mcpfit-{token_n}");
        let params = ToolsCallParams {
            name: self.name,
            arguments,
            meta: Some(serde_json::json!({"progressToken": &token_str})),
        };
        let params_value = serde_json::to_value(&params)
            .map_err(|e| McpfitError::internal(format!("encode tools/call params: {e}")))?;
        let (tx, rx) = mpsc::channel::<ProgressNotificationParams>(PROGRESS_CHANNEL_CAPACITY);
        let token = ProgressToken::String(token_str);
        // Register before sending so a fast server can't deliver progress
        // notifications before the routing entry exists.
        let missed = self.client.progress.register(token.clone(), tx);
        let pending = self.client.inner.start_call("tools/call", params_value);
        let cleanup = HandleCleanup {
            registry: self.client.progress.clone(),
            token,
        };
        Ok(ToolCallHandle::new(pending, rx, missed, Some(cleanup)))
    }
}

impl Client<SubprocessConnector> {
    pub async fn spawn(command: impl AsRef<OsStr>) -> Result<Self, McpfitError> {
        Self::connect(SubprocessConnector::new(command)).await
    }
}

fn spawn_notification_router(
    mut rx: broadcast::Receiver<InboundNotification>,
    progress: ProgressRegistry,
) -> JoinHandle<()> {
    fittings::tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(notification) => route_notification(&progress, notification),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
        progress.close_all();
    })
}

fn route_notification(progress: &ProgressRegistry, notification: InboundNotification) {
    if notification.method != "notifications/progress" {
        return;
    }
    let Some(params) = notification.params else {
        return;
    };
    let Ok(decoded) = serde_json::from_value::<ProgressNotificationParams>(params) else {
        return;
    };
    let Some(token) = ProgressToken::from_value(&decoded.progress_token) else {
        return;
    };
    progress.deliver(&token, decoded);
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
    use crate::content::ToolContent;
    use crate::protocol::{
        InitializeResult, ServerCapabilities, ServerInfo, ToolInfo, ToolsCapability,
    };
    use crate::response::ToolResponse;

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
    async fn notifications_delegates_to_fittings_subscription() {
        use std::time::Duration;

        use tokio::time::timeout;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let mut rx = client.notifications();

        server_transport
            .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\",\"params\":{\"foo\":1}}\n")
            .await
            .expect("send notification frame");

        let notif = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("notification arrives within timeout")
            .expect("broadcast not closed");
        assert_eq!(notif.method, "notifications/tools/list_changed");
        assert_eq!(notif.params, Some(json!({"foo": 1})));
    }

    #[tokio::test]
    async fn list_tools_sends_typed_request_and_decodes_result() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/list request");
            let request = parse_request_fixture(&frame).expect("decode request");
            assert_eq!(request.method, "tools/list");
            let id = request.id.expect("tools/list must carry id");
            assert_eq!(request.params, Some(json!({})));

            let response = success_response_line(
                id,
                json!({
                    "tools": [
                        {
                            "name": "echo",
                            "description": "Echo the input.",
                            "inputSchema": {"type": "object"},
                        },
                        {
                            "name": "add",
                            "inputSchema": {"type": "object"},
                        },
                    ],
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/list response");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let tools = client.list_tools().await.expect("list_tools succeeds");

        assert_eq!(
            tools,
            vec![
                ToolInfo {
                    name: "echo".into(),
                    description: Some("Echo the input.".into()),
                    input_schema: json!({"type": "object"}),
                },
                ToolInfo {
                    name: "add".into(),
                    description: None,
                    input_schema: json!({"type": "object"}),
                },
            ]
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn list_tools_surfaces_decode_failure_when_result_is_malformed() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/list request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("request id");
            let response = success_response_line(id, json!({"not": "a tools/list result"}))
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
            .list_tools()
            .await
            .expect_err("malformed result must surface as error");
        assert!(
            err.to_string().contains("decode tools/list result"),
            "unexpected error: {err}"
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn call_tool_raw_serializes_typed_args_and_decodes_response() {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AddArgs {
            a: i32,
            b: i32,
        }

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/call request");
            let request = parse_request_fixture(&frame).expect("decode request");
            assert_eq!(request.method, "tools/call");
            let id = request.id.expect("tools/call must carry id");
            assert_eq!(
                request.params,
                Some(json!({
                    "name": "add",
                    "arguments": {"a": 1, "b": 2},
                })),
            );

            let response = success_response_line(
                id,
                json!({
                    "content": [{"type": "text", "text": "3"}],
                    "isError": false,
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/call response");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let response = client
            .call_tool_raw("add", AddArgs { a: 1, b: 2 })
            .await
            .expect("call_tool_raw succeeds");

        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text("3")],
                structured_content: None,
                is_error: false,
            }
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn call_tool_raw_passes_through_is_error_without_mapping() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/call request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("tools/call must carry id");
            let response = success_response_line(
                id,
                json!({
                    "content": [{"type": "text", "text": "boom"}],
                    "isError": true,
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/call response");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let response = client
            .call_tool_raw("explode", json!({}))
            .await
            .expect("call_tool_raw must surface tool failure as Ok(ToolResponse)");

        assert!(
            response.is_error,
            "raw call must passthrough isError without mapping, got: {response:?}"
        );
        assert_eq!(response.content, vec![ToolContent::text("boom")]);

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn call_tool_returns_response_on_success() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/call request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("tools/call must carry id");
            let response = success_response_line(
                id,
                json!({
                    "content": [{"type": "text", "text": "ok"}],
                    "isError": false,
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/call response");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let response = client
            .call_tool("ping", json!({}))
            .await
            .expect("call_tool succeeds on isError: false");

        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text("ok")],
                structured_content: None,
                is_error: false,
            }
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn call_tool_maps_is_error_to_tool_failed() {
        use crate::error::McpfitError;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/call request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("tools/call must carry id");
            let response = success_response_line(
                id,
                json!({
                    "content": [{"type": "text", "text": "boom"}],
                    "isError": true,
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/call response");
        });

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");
        let err = client
            .call_tool("explode", json!({}))
            .await
            .expect_err("call_tool must surface isError as ToolFailed");

        match err {
            McpfitError::ToolFailed(response) => {
                assert!(response.is_error);
                assert_eq!(response.content, vec![ToolContent::text("boom")]);
            }
            other => panic!("expected ToolFailed, got: {other:?}"),
        }

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn drop_aborts_notification_router_task() {
        use std::time::Duration;

        use tokio::time::timeout;

        let (client_transport, _server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let router = client
            .router
            .as_ref()
            .expect("router task should exist while client is alive");
        assert!(
            !router.is_finished(),
            "router task must be running before drop"
        );
        let handle = router.abort_handle();
        drop(client);

        timeout(Duration::from_secs(1), async {
            while !handle.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("router task must finish promptly after Client drop");
    }

    #[tokio::test]
    async fn raw_notification_subscribers_still_receive_with_router_running() {
        use std::time::Duration;

        use tokio::time::timeout;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let mut rx = client.notifications();

        server_transport
            .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\",\"params\":{\"foo\":1}}\n")
            .await
            .expect("send notification frame");
        server_transport
            .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"progressToken\":\"abc\",\"progress\":1.0}}\n")
            .await
            .expect("send progress frame");

        let first = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("first notification arrives within timeout")
            .expect("broadcast not closed");
        assert_eq!(first.method, "notifications/tools/list_changed");

        let second = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("second notification arrives within timeout")
            .expect("broadcast not closed");
        assert_eq!(second.method, "notifications/progress");
    }

    #[tokio::test]
    async fn progress_token_from_value_accepts_string_and_number() {
        use super::ProgressToken;

        assert_eq!(
            ProgressToken::from_value(&json!("tok-1")),
            Some(ProgressToken::String("tok-1".into())),
        );
        assert_eq!(
            ProgressToken::from_value(&json!(42)),
            Some(ProgressToken::Number(42)),
        );
        assert_eq!(ProgressToken::from_value(&json!(null)), None);
        assert_eq!(ProgressToken::from_value(&json!({"x": 1})), None);
        assert_eq!(ProgressToken::from_value(&json!(1.5)), None);
    }

    #[tokio::test]
    async fn progress_registry_register_then_remove_round_trips_sender() {
        use fittings::tokio::sync::mpsc;

        use super::{ProgressRegistry, ProgressToken};
        use crate::protocol::ProgressNotificationParams;

        let registry = ProgressRegistry::default();
        let token = ProgressToken::String("call-1".into());
        let (tx, mut rx) = mpsc::channel::<ProgressNotificationParams>(4);

        registry.register(token.clone(), tx);

        let sender = registry
            .get(&token)
            .expect("registered sender should be retrievable");
        sender
            .send(ProgressNotificationParams {
                progress_token: json!("call-1"),
                progress: 0.5,
                total: None,
                message: None,
            })
            .await
            .expect("send through registered sender");
        let event = rx.recv().await.expect("receive routed progress event");
        assert_eq!(event.progress, 0.5);

        let removed = registry
            .remove(&token)
            .expect("remove should return previously registered sender");
        drop(removed);
        assert!(
            registry.get(&token).is_none(),
            "token must be absent after remove",
        );
    }

    #[tokio::test]
    async fn progress_registry_isolates_distinct_tokens() {
        use fittings::tokio::sync::mpsc;

        use super::{ProgressRegistry, ProgressToken};
        use crate::protocol::ProgressNotificationParams;

        let registry = ProgressRegistry::default();
        let token_a = ProgressToken::String("a".into());
        let token_b = ProgressToken::Number(7);
        let (tx_a, mut rx_a) = mpsc::channel::<ProgressNotificationParams>(4);
        let (tx_b, mut rx_b) = mpsc::channel::<ProgressNotificationParams>(4);

        registry.register(token_a.clone(), tx_a);
        registry.register(token_b.clone(), tx_b);

        registry
            .get(&token_a)
            .expect("token a present")
            .send(ProgressNotificationParams {
                progress_token: json!("a"),
                progress: 1.0,
                total: None,
                message: None,
            })
            .await
            .expect("send to token a");

        let received_a = rx_a.recv().await.expect("rx_a receives");
        assert_eq!(received_a.progress, 1.0);
        assert!(
            rx_b.try_recv().is_err(),
            "rx_b must not observe rx_a's event",
        );

        registry.remove(&token_a);
        assert!(registry.get(&token_a).is_none());
        assert!(
            registry.get(&token_b).is_some(),
            "removing token a must not affect token b",
        );
    }

    #[tokio::test]
    async fn client_exposes_progress_registry_clone_shared_with_router() {
        use fittings::tokio::sync::mpsc;

        use super::ProgressToken;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, _server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let registry = client.progress_registry().clone();
        let token = ProgressToken::Number(99);
        let (tx, _rx) = mpsc::channel::<ProgressNotificationParams>(1);
        registry.register(token.clone(), tx);

        assert!(
            client.progress_registry().get(&token).is_some(),
            "registry handed out by accessor must share state with the Client",
        );
    }

    #[tokio::test]
    async fn router_forwards_progress_notification_to_registered_token() {
        use std::time::Duration;

        use fittings::tokio::sync::mpsc;
        use tokio::time::timeout;

        use super::ProgressToken;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let token = ProgressToken::String("call-1".into());
        let (tx, mut rx) = mpsc::channel::<ProgressNotificationParams>(4);
        client.progress_registry().register(token, tx);

        server_transport
            .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"progressToken\":\"call-1\",\"progress\":0.25,\"total\":1.0,\"message\":\"quarter\"}}\n")
            .await
            .expect("send progress frame");

        let event = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("router routes within timeout")
            .expect("progress channel not closed");
        assert_eq!(event.progress_token, json!("call-1"));
        assert_eq!(event.progress, 0.25);
        assert_eq!(event.total, Some(1.0));
        assert_eq!(event.message.as_deref(), Some("quarter"));
    }

    #[tokio::test]
    async fn router_drops_progress_notifications_for_unknown_tokens() {
        use std::time::Duration;

        use fittings::tokio::sync::mpsc;
        use tokio::time::timeout;

        use super::ProgressToken;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let registered = ProgressToken::String("known".into());
        let (tx, mut rx) = mpsc::channel::<ProgressNotificationParams>(4);
        client.progress_registry().register(registered, tx);

        server_transport
            .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"progressToken\":\"stranger\",\"progress\":0.5}}\n")
            .await
            .expect("send unknown-token progress frame");
        server_transport
            .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"progressToken\":\"known\",\"progress\":0.75}}\n")
            .await
            .expect("send known-token progress frame");

        let event = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("router routes within timeout")
            .expect("progress channel not closed");
        assert_eq!(event.progress, 0.75);
        assert_eq!(event.progress_token, json!("known"));
        assert!(
            rx.try_recv().is_err(),
            "registered token must not see the unknown-token event",
        );
    }

    #[tokio::test]
    async fn router_records_missed_progress_when_per_call_buffer_is_full() {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        use fittings::tokio::sync::mpsc;
        use tokio::time::timeout;

        use super::{ProgressToken, PROGRESS_CHANNEL_CAPACITY};
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, mut server_transport) = MemoryTransport::pair(256);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let token = ProgressToken::String("flooded".into());
        let (tx, rx) = mpsc::channel::<ProgressNotificationParams>(PROGRESS_CHANNEL_CAPACITY);
        let missed = client.progress_registry().register(token.clone(), tx);

        let extra = 8;
        let total_events = PROGRESS_CHANNEL_CAPACITY + extra;
        for i in 0..total_events {
            let frame = format!(
                "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{{\"progressToken\":\"flooded\",\"progress\":{i}}}}}\n"
            );
            server_transport
                .send(frame.as_bytes())
                .await
                .expect("send progress frame");
        }

        let expected_missed = extra as u64;
        timeout(Duration::from_secs(2), async {
            while missed.load(Ordering::Relaxed) < expected_missed {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("router stays alive and missed counter reaches expected value");

        assert_eq!(
            missed.load(Ordering::Relaxed),
            expected_missed,
            "exactly {expected_missed} events should have overflowed the per-call buffer",
        );
        assert!(
            client
                .progress_registry()
                .missed_counter(&token)
                .is_some(),
            "missed counter handle must remain accessible while token is registered",
        );

        drop(rx);
    }

    #[tokio::test]
    async fn client_drop_closes_active_progress_streams() {
        use std::time::Duration;

        use fittings::tokio::sync::mpsc;
        use tokio::time::timeout;

        use super::ProgressToken;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, _server_transport) = MemoryTransport::pair(8);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let token = ProgressToken::String("call-1".into());
        let (tx, mut rx) = mpsc::channel::<ProgressNotificationParams>(4);
        client.progress_registry().register(token, tx);

        drop(client);

        let received = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("progress receiver must observe close after Client drop");
        assert!(
            received.is_none(),
            "progress channel must close when Client is dropped, got: {received:?}",
        );
    }

    #[tokio::test]
    async fn tool_call_handle_awaits_final_response_on_success() {
        use std::sync::atomic::AtomicU64;

        use fittings::{client::Client as FittingsClient, tokio::sync::mpsc};

        use super::ToolCallHandle;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/call request");
            let request = parse_request_fixture(&frame).expect("decode request");
            assert_eq!(request.method, "tools/call");
            let id = request.id.expect("tools/call must carry id");
            let response = success_response_line(
                id,
                json!({
                    "content": [{"type": "text", "text": "ok"}],
                    "isError": false,
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/call response");
        });

        let fittings_client = FittingsClient::connect(OneShotConnector::new(client_transport))
            .await
            .expect("fittings client connects");
        let pending = fittings_client.start_call(
            "tools/call",
            json!({"name": "ping", "arguments": {}}),
        );
        let (_tx, rx) = mpsc::channel::<ProgressNotificationParams>(4);
        let missed = Arc::new(AtomicU64::new(0));
        let handle = ToolCallHandle::new(pending, rx, missed, None);

        let response = handle.await.expect("handle resolves to ToolResponse");

        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text("ok")],
                structured_content: None,
                is_error: false,
            }
        );

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn tool_call_handle_maps_is_error_to_tool_failed() {
        use std::sync::atomic::AtomicU64;

        use fittings::{client::Client as FittingsClient, tokio::sync::mpsc};

        use super::ToolCallHandle;
        use crate::error::McpfitError;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);

        let server = tokio::spawn(async move {
            let frame = server_transport.recv().await.expect("tools/call request");
            let request = parse_request_fixture(&frame).expect("decode request");
            let id = request.id.expect("tools/call must carry id");
            let response = success_response_line(
                id,
                json!({
                    "content": [{"type": "text", "text": "boom"}],
                    "isError": true,
                }),
            )
            .expect("encode response");
            server_transport
                .send(&response)
                .await
                .expect("send tools/call response");
        });

        let fittings_client = FittingsClient::connect(OneShotConnector::new(client_transport))
            .await
            .expect("fittings client connects");
        let pending = fittings_client.start_call(
            "tools/call",
            json!({"name": "explode", "arguments": {}}),
        );
        let (_tx, rx) = mpsc::channel::<ProgressNotificationParams>(4);
        let missed = Arc::new(AtomicU64::new(0));
        let handle = ToolCallHandle::new(pending, rx, missed, None);

        let err = handle
            .await
            .expect_err("handle must surface isError as ToolFailed");

        match err {
            McpfitError::ToolFailed(response) => {
                assert!(response.is_error);
                assert_eq!(response.content, vec![ToolContent::text("boom")]);
            }
            other => panic!("expected ToolFailed, got: {other:?}"),
        }

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn tool_call_handle_missed_progress_count_reads_shared_counter() {
        use std::sync::atomic::{AtomicU64, Ordering};

        use fittings::{client::Client as FittingsClient, tokio::sync::mpsc};

        use super::ToolCallHandle;
        use crate::protocol::ProgressNotificationParams;

        let (client_transport, _server_transport) = MemoryTransport::pair(8);
        let fittings_client = FittingsClient::connect(OneShotConnector::new(client_transport))
            .await
            .expect("fittings client connects");
        let pending = fittings_client.start_call("tools/call", json!({}));
        let (_tx, rx) = mpsc::channel::<ProgressNotificationParams>(4);
        let missed = Arc::new(AtomicU64::new(0));
        let handle = ToolCallHandle::new(pending, rx, missed.clone(), None);

        assert_eq!(handle.missed_progress_count(), 0);
        missed.store(7, Ordering::Relaxed);
        assert_eq!(handle.missed_progress_count(), 7);
    }

    #[tokio::test]
    async fn call_tool_with_progress_injects_token_and_routes_progress_to_handle() {
        use std::time::Duration;

        use tokio::time::timeout;

        let (client_transport, mut server_transport) = MemoryTransport::pair(16);

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let mut handle = client
            .call_tool_with_progress("work", json!({"n": 5}))
            .start()
            .await
            .expect("start progress-enabled call");

        let frame = server_transport.recv().await.expect("tools/call request");
        let request = parse_request_fixture(&frame).expect("decode request");
        assert_eq!(request.method, "tools/call");
        let id = request.id.expect("tools/call must carry id");
        let params = request.params.expect("tools/call must carry params");
        assert_eq!(params.get("name"), Some(&json!("work")));
        assert_eq!(params.get("arguments"), Some(&json!({"n": 5})));
        let token = params
            .get("_meta")
            .and_then(|m| m.get("progressToken"))
            .cloned()
            .expect("_meta.progressToken must be injected by builder");
        assert!(
            !token.is_null(),
            "injected progress token must not be null, got: {token:?}",
        );

        let token_json = serde_json::to_string(&token).expect("encode token");
        let progress_frame = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{{\"progressToken\":{token_json},\"progress\":0.25}}}}\n"
        );
        server_transport
            .send(progress_frame.as_bytes())
            .await
            .expect("send progress frame");

        let event = timeout(Duration::from_secs(1), handle.progress().recv())
            .await
            .expect("progress arrives within timeout")
            .expect("progress channel not closed");
        assert_eq!(event.progress, 0.25);
        assert_eq!(event.progress_token, token);

        let response_line = success_response_line(
            id,
            json!({
                "content": [{"type": "text", "text": "done"}],
                "isError": false,
            }),
        )
        .expect("encode response");
        server_transport
            .send(&response_line)
            .await
            .expect("send tools/call response");

        let response = handle.await.expect("handle resolves to ToolResponse");
        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text("done")],
                structured_content: None,
                is_error: false,
            }
        );
    }

    #[tokio::test]
    async fn concurrent_progress_calls_receive_only_their_own_progress_events() {
        use std::time::Duration;

        use tokio::time::timeout;

        let (client_transport, mut server_transport) = MemoryTransport::pair(32);

        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let mut handle_a = client
            .call_tool_with_progress("a", json!({}))
            .start()
            .await
            .expect("start call A");
        let mut handle_b = client
            .call_tool_with_progress("b", json!({}))
            .start()
            .await
            .expect("start call B");

        let frame_a = server_transport.recv().await.expect("call A request");
        let req_a = parse_request_fixture(&frame_a).expect("decode A");
        let id_a = req_a.id.expect("A id");
        let token_a = req_a
            .params
            .as_ref()
            .and_then(|p| p.get("_meta"))
            .and_then(|m| m.get("progressToken"))
            .cloned()
            .expect("A progress token");

        let frame_b = server_transport.recv().await.expect("call B request");
        let req_b = parse_request_fixture(&frame_b).expect("decode B");
        let id_b = req_b.id.expect("B id");
        let token_b = req_b
            .params
            .as_ref()
            .and_then(|p| p.get("_meta"))
            .and_then(|m| m.get("progressToken"))
            .cloned()
            .expect("B progress token");

        assert_ne!(
            token_a, token_b,
            "concurrent calls must get distinct progress tokens",
        );

        let token_a_json = serde_json::to_string(&token_a).expect("encode A token");
        let token_b_json = serde_json::to_string(&token_b).expect("encode B token");
        let progress_a = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{{\"progressToken\":{token_a_json},\"progress\":0.1}}}}\n"
        );
        let progress_b = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{{\"progressToken\":{token_b_json},\"progress\":0.9}}}}\n"
        );
        server_transport
            .send(progress_a.as_bytes())
            .await
            .expect("send A progress");
        server_transport
            .send(progress_b.as_bytes())
            .await
            .expect("send B progress");

        let event_a = timeout(Duration::from_secs(1), handle_a.progress().recv())
            .await
            .expect("A progress arrives")
            .expect("A channel open");
        assert_eq!(event_a.progress, 0.1);
        assert_eq!(event_a.progress_token, token_a);

        let event_b = timeout(Duration::from_secs(1), handle_b.progress().recv())
            .await
            .expect("B progress arrives")
            .expect("B channel open");
        assert_eq!(event_b.progress, 0.9);
        assert_eq!(event_b.progress_token, token_b);

        assert!(
            handle_a.progress().try_recv().is_err(),
            "A handle must not observe B's progress event",
        );
        assert!(
            handle_b.progress().try_recv().is_err(),
            "B handle must not observe A's progress event",
        );

        let resp_a = success_response_line(
            id_a,
            json!({"content": [{"type": "text", "text": "A"}], "isError": false}),
        )
        .expect("encode A response");
        let resp_b = success_response_line(
            id_b,
            json!({"content": [{"type": "text", "text": "B"}], "isError": false}),
        )
        .expect("encode B response");
        server_transport
            .send(&resp_a)
            .await
            .expect("send A response");
        server_transport
            .send(&resp_b)
            .await
            .expect("send B response");

        let response_a = handle_a.await.expect("A resolves");
        let response_b = handle_b.await.expect("B resolves");
        assert_eq!(response_a.content, vec![ToolContent::text("A")]);
        assert_eq!(response_b.content, vec![ToolContent::text("B")]);
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

    #[tokio::test]
    async fn awaiting_progress_handle_removes_registry_entry() {
        use super::ProgressToken;

        let (client_transport, mut server_transport) = MemoryTransport::pair(16);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let handle = client
            .call_tool_with_progress("work", json!({}))
            .start()
            .await
            .expect("start progress-enabled call");

        let frame = server_transport.recv().await.expect("tools/call request");
        let request = parse_request_fixture(&frame).expect("decode request");
        let id = request.id.expect("tools/call must carry id");
        let token = request
            .params
            .as_ref()
            .and_then(|p| p.get("_meta"))
            .and_then(|m| m.get("progressToken"))
            .cloned()
            .expect("progress token injected");
        let progress_token =
            ProgressToken::from_value(&token).expect("token convertible");
        assert!(
            client.progress_registry().get(&progress_token).is_some(),
            "registry must hold token while handle is alive",
        );

        let response_line = success_response_line(
            id,
            json!({"content": [{"type": "text", "text": "done"}], "isError": false}),
        )
        .expect("encode response");
        server_transport
            .send(&response_line)
            .await
            .expect("send response");

        let _response = handle.await.expect("handle resolves");
        assert!(
            client.progress_registry().get(&progress_token).is_none(),
            "registry entry must be removed after handle completes",
        );
    }

    #[tokio::test]
    async fn dropping_progress_handle_removes_registry_entry() {
        use super::ProgressToken;

        let (client_transport, mut server_transport) = MemoryTransport::pair(16);
        let client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client connects");

        let handle = client
            .call_tool_with_progress("work", json!({}))
            .start()
            .await
            .expect("start progress-enabled call");

        let frame = server_transport.recv().await.expect("tools/call request");
        let request = parse_request_fixture(&frame).expect("decode request");
        let token = request
            .params
            .as_ref()
            .and_then(|p| p.get("_meta"))
            .and_then(|m| m.get("progressToken"))
            .cloned()
            .expect("progress token injected");
        let progress_token =
            ProgressToken::from_value(&token).expect("token convertible");
        assert!(
            client.progress_registry().get(&progress_token).is_some(),
            "registry must hold token while handle is alive",
        );

        drop(handle);
        assert!(
            client.progress_registry().get(&progress_token).is_none(),
            "registry entry must be removed when handle is dropped without awaiting",
        );
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
