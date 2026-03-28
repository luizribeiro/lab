use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use fittings::serde_json::{json, Value};
use fittings::{FittingsError, Result, Transport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCallParams {
    pub name: String,
    #[serde(default = "empty_object")]
    pub arguments: Value,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRegisterParams {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub response_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRegisterResult {
    pub tool: Tool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledNotificationParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<fittings::wire::types::JsonRpcId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCallResult {
    pub content: Vec<ToolContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolsCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            structured_content: None,
            is_error: false,
        }
    }

    pub fn text_with_structured(
        text: impl Into<String>,
        structured_content: Value,
    ) -> Result<Self> {
        if !structured_content.is_object() {
            return Err(FittingsError::invalid_params(
                "`structuredContent` must be a JSON object",
            ));
        }

        Ok(Self {
            content: vec![ToolContent::Text { text: text.into() }],
            structured_content: Some(structured_content),
            is_error: false,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    reason: Arc<Mutex<Option<String>>>,
}

impl CancellationToken {
    pub fn cancel(&self, reason: Option<String>) {
        self.cancelled.store(true, Ordering::Release);
        let mut cancellation_reason = self
            .reason
            .lock()
            .expect("cancellation reason mutex should not be poisoned");
        if cancellation_reason.is_none() {
            *cancellation_reason = reason;
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallContext {
    cancellation: CancellationToken,
}

impl ToolCallContext {
    pub fn new(cancellation: CancellationToken) -> Self {
        Self { cancellation }
    }

    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }
}

type ToolHandler = Arc<dyn Fn(Value, &ToolCallContext) -> Result<ToolsCallResult> + Send + Sync>;

struct RegisteredTool {
    tool: Tool,
    handler: ToolHandler,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Value, &ToolCallContext) -> Result<ToolsCallResult> + Send + Sync + 'static,
    {
        let name = name.into();
        if self.tools.contains_key(&name) {
            return Err(FittingsError::invalid_request(format!(
                "tool `{name}` is already registered"
            )));
        }

        self.tools.insert(
            name.clone(),
            RegisteredTool {
                tool: Tool {
                    name,
                    description: Some(description.into()),
                    input_schema,
                },
                handler: Arc::new(handler),
            },
        );

        Ok(())
    }

    pub fn register_static_text_tool(
        &mut self,
        name: impl Into<String>,
        description: Option<String>,
        response_text: impl Into<String>,
    ) -> Result<Tool> {
        let name = name.into();
        let response_text = response_text.into();
        let description = description.unwrap_or_else(|| "Runtime registered tool".to_string());

        self.register(
            name.clone(),
            description,
            json!({
                "type": "object",
                "additionalProperties": false
            }),
            move |_arguments, _context| Ok(ToolsCallResult::text(response_text.clone())),
        )?;

        let tool = self
            .tools
            .get(&name)
            .map(|entry| entry.tool.clone())
            .expect("newly registered tool should exist");

        Ok(tool)
    }

    pub fn list(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|entry| entry.tool.clone())
            .collect()
    }

    fn handler_for(&self, name: &str) -> Result<ToolHandler> {
        self.tools
            .get(name)
            .map(|entry| Arc::clone(&entry.handler))
            .ok_or_else(|| FittingsError::method_not_found(format!("unknown tool `{name}`")))
    }

    #[cfg(test)]
    pub fn execute(
        &self,
        params: ToolsCallParams,
        context: &ToolCallContext,
    ) -> Result<ToolsCallResult> {
        if !params.arguments.is_object() {
            return Err(FittingsError::invalid_params(
                "`arguments` must be a JSON object",
            ));
        }

        let handler = self.handler_for(&params.name)?;
        handler(params.arguments, context)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerNotification {
    pub method: String,
    pub params: Value,
}

#[fittings::service]
pub trait McpService {
    /// Minimal MCP initialize handshake (stdio-oriented baseline).
    #[fittings::method(name = "initialize")]
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult>;

    /// Client notification sent after successful initialize handshake.
    #[fittings::method(name = "notifications/initialized")]
    async fn initialized(&self, params: Value) -> Result<Value>;

    /// Returns the tools exposed by this process.
    #[fittings::method(name = "tools/list")]
    async fn list_tools(&self, params: Value) -> Result<ToolsListResult>;

    /// Executes a named tool with JSON arguments.
    #[fittings::method(name = "tools/call")]
    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolsCallResult>;

    /// Registers a simple runtime tool and notifies active clients that tools/list changed.
    #[fittings::method(name = "tools/register")]
    async fn register_tool(&self, params: ToolsRegisterParams) -> Result<ToolsRegisterResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionLifecycle {
    AwaitingInitialize,
    AwaitingInitializedNotification,
    Running,
}

pub struct McpServiceImpl {
    registry: Mutex<ToolRegistry>,
    lifecycle: Mutex<SessionLifecycle>,
    list_changed_enabled: bool,
    pending_notifications: Mutex<Vec<ServerNotification>>,
}

impl McpServiceImpl {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry: Mutex::new(registry),
            lifecycle: Mutex::new(SessionLifecycle::AwaitingInitialize),
            list_changed_enabled: false,
            pending_notifications: Mutex::new(Vec::new()),
        }
    }

    pub fn with_tools_list_changed(mut self, enabled: bool) -> Self {
        self.list_changed_enabled = enabled;
        self
    }

    pub fn drain_notifications(&self) -> Vec<ServerNotification> {
        let mut notifications = self
            .pending_notifications
            .lock()
            .expect("pending notifications mutex should not be poisoned");
        notifications.drain(..).collect()
    }

    fn enqueue_tools_list_changed_notification(&self) {
        if !self.list_changed_enabled {
            return;
        }

        let lifecycle = self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned");
        if *lifecycle != SessionLifecycle::Running {
            return;
        }

        drop(lifecycle);

        self.pending_notifications
            .lock()
            .expect("pending notifications mutex should not be poisoned")
            .push(ServerNotification {
                method: "notifications/tools/list_changed".to_string(),
                params: json!({}),
            });
    }

    async fn call_tool_with_context(
        &self,
        params: ToolsCallParams,
        context: ToolCallContext,
    ) -> Result<ToolsCallResult> {
        if !params.arguments.is_object() {
            return Err(FittingsError::invalid_params(
                "`arguments` must be a JSON object",
            ));
        }

        let handler = {
            let registry = self
                .registry
                .lock()
                .expect("tool registry mutex should not be poisoned");
            registry.handler_for(&params.name)?
        };

        handler(params.arguments, &context)
    }

    #[cfg(test)]
    fn lifecycle_state(&self) -> SessionLifecycle {
        *self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned")
    }
}

impl Default for McpServiceImpl {
    fn default() -> Self {
        Self::new(ToolRegistry::new())
    }
}

impl McpService for McpServiceImpl {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        {
            let mut lifecycle = self
                .lifecycle
                .lock()
                .expect("session lifecycle mutex should not be poisoned");
            *lifecycle = SessionLifecycle::AwaitingInitializedNotification;
        }

        Ok(InitializeResult {
            protocol_version: params.protocol_version,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: self.list_changed_enabled.then_some(true),
                }),
            },
            server_info: ServerInfo {
                name: "fittings-mcp-example".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }

    async fn initialized(&self, _params: Value) -> Result<Value> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned");

        match *lifecycle {
            SessionLifecycle::AwaitingInitialize => Err(FittingsError::invalid_request(
                "received notifications/initialized before initialize",
            )),
            SessionLifecycle::AwaitingInitializedNotification => {
                *lifecycle = SessionLifecycle::Running;
                Ok(Value::Null)
            }
            SessionLifecycle::Running => Ok(Value::Null),
        }
    }

    async fn list_tools(&self, _params: Value) -> Result<ToolsListResult> {
        let registry = self
            .registry
            .lock()
            .expect("tool registry mutex should not be poisoned");

        Ok(ToolsListResult {
            tools: registry.list(),
        })
    }

    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolsCallResult> {
        self.call_tool_with_context(params, ToolCallContext::default())
            .await
    }

    async fn register_tool(&self, params: ToolsRegisterParams) -> Result<ToolsRegisterResult> {
        if params.name.trim().is_empty() {
            return Err(FittingsError::invalid_params("`name` must not be empty"));
        }

        let tool = {
            let mut registry = self
                .registry
                .lock()
                .expect("tool registry mutex should not be poisoned");
            registry.register_static_text_tool(
                params.name,
                params.description,
                params.response_text,
            )?
        };

        self.enqueue_tools_list_changed_notification();

        Ok(ToolsRegisterResult { tool })
    }
}

pub async fn serve_stdio(service: Arc<McpServiceImpl>) -> Result<()> {
    let mut transport = fittings::from_process_stdio(1_048_576);
    let mut input_closed = false;
    let mut workers = tokio::task::JoinSet::<(
        fittings::wire::types::JsonRpcId,
        Result<Value>,
        CancellationToken,
    )>::new();
    let mut in_flight = HashMap::<fittings::wire::types::JsonRpcId, CancellationToken>::new();

    loop {
        if input_closed && workers.is_empty() {
            return Ok(());
        }

        tokio::select! {
            maybe_joined = workers.join_next(), if !workers.is_empty() => {
                match maybe_joined {
                    Some(Ok((id, result, cancellation))) => {
                        in_flight.remove(&id);

                        if !cancellation.is_cancelled() {
                            let response = match result {
                                Ok(value) => fittings::ResponseEnvelope::success(id, value),
                                Err(error) => fittings::to_error_envelope(id, error),
                            };
                            send_response(&mut transport, &response).await?;
                        }

                        send_pending_notifications(&mut transport, &service).await?;
                    }
                    Some(Err(error)) => {
                        return Err(FittingsError::internal(format!(
                            "failed to join request worker: {error}"
                        )));
                    }
                    None => {}
                }
            }
            recv_result = transport.recv(), if !input_closed => {
                match recv_result {
                    Ok(frame) => {
                        let request = match fittings::decode_request_line(&frame) {
                            Ok(request) => request,
                            Err(error) => {
                                let (id, err) = map_decode_error(error);
                                let response = fittings::to_error_envelope(id, err);
                                send_response(&mut transport, &response).await?;
                                continue;
                            }
                        };

                        if request.method == "notifications/cancelled" && request.id.is_none() {
                            handle_cancellation_notification(
                                request.params.unwrap_or(Value::Null),
                                &in_flight,
                            );
                            continue;
                        }

                        if let Some(id) = request.id {
                            if in_flight.contains_key(&id) {
                                let response = fittings::to_error_envelope(
                                    id,
                                    FittingsError::invalid_request("duplicate in-flight request id"),
                                );
                                send_response(&mut transport, &response).await?;
                                continue;
                            }

                            if request.method == "tools/call" {
                                let method = request.method;
                                let params = request.params.unwrap_or(Value::Null);
                                let cancellation = CancellationToken::default();
                                in_flight.insert(id.clone(), cancellation.clone());

                                let service = Arc::clone(&service);
                                workers.spawn(async move {
                                    let context = ToolCallContext::new(cancellation.clone());
                                    let result = dispatch_method(&service, &method, params, context).await;
                                    (id, result, cancellation)
                                });
                                continue;
                            }

                            let params = request.params.unwrap_or(Value::Null);
                            let context = ToolCallContext::default();
                            let result =
                                dispatch_method(&service, &request.method, params, context).await;
                            let response = match result {
                                Ok(value) => fittings::ResponseEnvelope::success(id, value),
                                Err(error) => fittings::to_error_envelope(id, error),
                            };
                            send_response(&mut transport, &response).await?;
                            send_pending_notifications(&mut transport, &service).await?;
                            continue;
                        }

                        let params = request.params.unwrap_or(Value::Null);
                        let context = ToolCallContext::default();
                        let _ = dispatch_method(&service, &request.method, params, context).await;
                        send_pending_notifications(&mut transport, &service).await?;
                    }
                    Err(error) if is_graceful_eof(&error) => {
                        input_closed = true;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }
}

fn handle_cancellation_notification(
    params: Value,
    in_flight: &HashMap<fittings::wire::types::JsonRpcId, CancellationToken>,
) {
    let Ok(notification) = fittings::serde_json::from_value::<CancelledNotificationParams>(params)
    else {
        return;
    };

    let Some(request_id) = notification.request_id else {
        return;
    };

    if let Some(token) = in_flight.get(&request_id) {
        token.cancel(notification.reason);
    }
}

async fn send_pending_notifications(
    transport: &mut impl Transport,
    service: &McpServiceImpl,
) -> Result<()> {
    for notification in service.drain_notifications() {
        let frame =
            fittings::RequestEnvelope::notification(notification.method, Some(notification.params));
        let mut encoded = fittings::serde_json::to_vec(&frame).map_err(|error| {
            FittingsError::internal(format!("failed to encode notification frame: {error}"))
        })?;
        encoded.push(b'\n');
        transport.send(&encoded).await?;
    }

    Ok(())
}

async fn send_response(
    transport: &mut impl Transport,
    response: &fittings::ResponseEnvelope,
) -> Result<()> {
    let encoded = fittings::encode_response_line(response).map_err(|error| {
        FittingsError::internal(format!("failed to encode response frame: {error}"))
    })?;
    transport.send(&encoded).await
}

async fn dispatch_method(
    service: &McpServiceImpl,
    method: &str,
    params: Value,
    context: ToolCallContext,
) -> Result<Value> {
    match method {
        "initialize" => {
            let decoded: InitializeParams = decode_params(method, params)?;
            let result = service.initialize(decoded).await?;
            fittings::serde_json::to_value(result).map_err(|error| {
                FittingsError::internal(format!(
                    "failed to encode result for method `{method}`: {error}"
                ))
            })
        }
        "notifications/initialized" => {
            let decoded: Value = decode_params(method, params)?;
            let result = service.initialized(decoded).await?;
            fittings::serde_json::to_value(result).map_err(|error| {
                FittingsError::internal(format!(
                    "failed to encode result for method `{method}`: {error}"
                ))
            })
        }
        "tools/list" => {
            let decoded: Value = decode_params(method, params)?;
            let result = service.list_tools(decoded).await?;
            fittings::serde_json::to_value(result).map_err(|error| {
                FittingsError::internal(format!(
                    "failed to encode result for method `{method}`: {error}"
                ))
            })
        }
        "tools/call" => {
            let decoded: ToolsCallParams = decode_params(method, params)?;
            let result = service.call_tool_with_context(decoded, context).await?;
            fittings::serde_json::to_value(result).map_err(|error| {
                FittingsError::internal(format!(
                    "failed to encode result for method `{method}`: {error}"
                ))
            })
        }
        "tools/register" => {
            let decoded: ToolsRegisterParams = decode_params(method, params)?;
            let result = service.register_tool(decoded).await?;
            fittings::serde_json::to_value(result).map_err(|error| {
                FittingsError::internal(format!(
                    "failed to encode result for method `{method}`: {error}"
                ))
            })
        }
        _ => Err(FittingsError::method_not_found(method.to_string())),
    }
}

fn decode_params<T>(method: &str, params: Value) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    fittings::serde_json::from_value(params).map_err(|error| {
        FittingsError::invalid_params(format!(
            "failed to decode params for method `{method}`: {error}"
        ))
    })
}

fn map_decode_error(
    error: fittings::WireDecodeError,
) -> (fittings::wire::types::JsonRpcId, FittingsError) {
    match error {
        fittings::WireDecodeError::Parse(message) => (
            fittings::wire::types::JsonRpcId::Null,
            FittingsError::parse_error(message),
        ),
        fittings::WireDecodeError::InvalidRequest { message, id } => (
            id.unwrap_or(fittings::wire::types::JsonRpcId::Null),
            FittingsError::invalid_request(message),
        ),
    }
}

fn is_graceful_eof(error: &FittingsError) -> bool {
    matches!(error, FittingsError::Transport(message) if message == "end of input" || message.ends_with("input closed"))
}

fn empty_object() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_schema_contains_mcp_method_names() {
        let schema = mcp_service_schema();
        let method_names: Vec<_> = schema
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect();

        assert_eq!(schema.name, "mcp-service");
        assert_eq!(
            method_names,
            vec![
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/call",
                "tools/register"
            ]
        );
    }

    #[test]
    fn initialize_result_serializes_as_camel_case() {
        let result = InitializeResult {
            protocol_version: "2025-01-01".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
            },
            server_info: ServerInfo {
                name: "server".to_string(),
                version: "1.2.3".to_string(),
            },
        };

        let encoded =
            fittings::serde_json::to_value(&result).expect("initialize result should serialize");

        assert_eq!(encoded["protocolVersion"], "2025-01-01");
        assert_eq!(encoded["serverInfo"]["name"], "server");
        assert_eq!(encoded["capabilities"]["tools"]["listChanged"], true);
    }

    #[test]
    fn tools_call_params_defaults_arguments_to_object() {
        let params: ToolsCallParams = fittings::serde_json::from_value(json!({
            "name": "echo"
        }))
        .expect("params should deserialize");

        assert_eq!(params.name, "echo");
        assert_eq!(params.arguments, json!({}));
    }

    #[test]
    fn tools_call_result_supports_text_and_structured_content() {
        let result = ToolsCallResult::text_with_structured(
            "2 + 3 = 5",
            json!({
                "a": 2,
                "b": 3,
                "sum": 5
            }),
        )
        .expect("tools/call result with structured content should be valid");

        let encoded = fittings::serde_json::to_value(result)
            .expect("tools/call result should serialize to JSON");

        assert_eq!(encoded["content"][0]["type"], "text");
        assert_eq!(encoded["content"][0]["text"], "2 + 3 = 5");
        assert_eq!(encoded["structuredContent"]["sum"], 5);
        assert_eq!(encoded["isError"], false);
    }

    #[test]
    fn tools_call_result_rejects_non_object_structured_content() {
        let result = ToolsCallResult::text_with_structured("invalid", json!([1, 2, 3]));

        assert!(matches!(result, Err(FittingsError::InvalidParams(_))));
    }

    #[test]
    fn registry_lists_tools_and_executes_tool_handler() {
        let mut registry = ToolRegistry::new();
        registry
            .register(
                "sum",
                "Sums two numbers",
                json!({"type": "object"}),
                |arguments, _context| {
                    let a = arguments["a"].as_i64().ok_or_else(|| {
                        FittingsError::invalid_params("`arguments.a` must be an integer")
                    })?;
                    let b = arguments["b"].as_i64().ok_or_else(|| {
                        FittingsError::invalid_params("`arguments.b` must be an integer")
                    })?;
                    Ok(ToolsCallResult::text((a + b).to_string()))
                },
            )
            .expect("register should succeed");

        let listed = registry.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "sum");

        let result = registry
            .execute(
                ToolsCallParams {
                    name: "sum".to_string(),
                    arguments: json!({"a": 2, "b": 3}),
                    meta: None,
                },
                &ToolCallContext::default(),
            )
            .expect("tool execution should succeed");

        assert_eq!(result.content.len(), 1);
        assert!(matches!(
            &result.content[0],
            ToolContent::Text { text } if text == "5"
        ));
    }

    #[test]
    fn tool_handlers_can_observe_cancellation_context() {
        let mut registry = ToolRegistry::new();
        registry
            .register(
                "cancel-aware",
                "Observes cancellation",
                json!({"type": "object"}),
                |_arguments, context| {
                    if context.cancellation().is_cancelled() {
                        return Ok(ToolsCallResult::text("cancelled"));
                    }
                    Ok(ToolsCallResult::text("running"))
                },
            )
            .expect("register should succeed");

        let token = CancellationToken::default();
        token.cancel(Some("test".to_string()));
        let context = ToolCallContext::new(token);

        let result = registry
            .execute(
                ToolsCallParams {
                    name: "cancel-aware".to_string(),
                    arguments: json!({}),
                    meta: None,
                },
                &context,
            )
            .expect("tool execution should succeed");

        assert!(matches!(
            &result.content[0],
            ToolContent::Text { text } if text == "cancelled"
        ));
    }

    #[test]
    fn registry_reports_unknown_tool_and_bad_arguments() {
        let registry = ToolRegistry::new();

        let unknown = registry.execute(
            ToolsCallParams {
                name: "missing".to_string(),
                arguments: json!({}),
                meta: None,
            },
            &ToolCallContext::default(),
        );
        assert!(matches!(unknown, Err(FittingsError::MethodNotFound(_))));

        let mut registry = ToolRegistry::new();
        registry
            .register(
                "noop",
                "No-op tool",
                json!({"type": "object"}),
                |_, _context| Ok(ToolsCallResult::text("ok")),
            )
            .expect("register should succeed");

        let invalid = registry.execute(
            ToolsCallParams {
                name: "noop".to_string(),
                arguments: json!([1, 2, 3]),
                meta: None,
            },
            &ToolCallContext::default(),
        );
        assert!(matches!(invalid, Err(FittingsError::InvalidParams(_))));
    }

    #[test]
    fn registry_rejects_duplicate_tool_names() {
        let mut registry = ToolRegistry::new();
        registry
            .register("echo", "Echo", json!({"type": "object"}), |_, _context| {
                Ok(ToolsCallResult::text("one"))
            })
            .expect("first registration should succeed");

        let duplicate = registry.register(
            "echo",
            "Echo again",
            json!({"type": "object"}),
            |_, _context| Ok(ToolsCallResult::text("two")),
        );

        assert!(matches!(duplicate, Err(FittingsError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn default_service_supports_initialize_and_empty_tool_list() {
        let service = McpServiceImpl::default();

        let initialize = service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");
        assert_eq!(initialize.protocol_version, "2024-11-05");
        assert!(initialize.capabilities.tools.is_some());
        assert_eq!(
            initialize
                .capabilities
                .tools
                .expect("tools capability should be present")
                .list_changed,
            None
        );

        let listed = service
            .list_tools(fittings::serde_json::Value::Null)
            .await
            .expect("tools/list should succeed");
        assert!(listed.tools.is_empty());

        let called = service
            .call_tool(ToolsCallParams {
                name: "echo".to_string(),
                arguments: json!({"message": "hello"}),
                meta: None,
            })
            .await;

        assert!(matches!(called, Err(FittingsError::MethodNotFound(_))));
    }

    #[tokio::test]
    async fn initialized_before_initialize_is_invalid_request() {
        let service = McpServiceImpl::default();

        let result = service.initialized(Value::Null).await;

        assert!(matches!(result, Err(FittingsError::InvalidRequest(_))));
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitialize,
            "state should remain unchanged"
        );
    }

    #[tokio::test]
    async fn initialize_and_initialized_notification_advance_session_lifecycle() {
        let service = McpServiceImpl::default();
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitialize
        );

        service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitializedNotification
        );

        service
            .initialized(Value::Null)
            .await
            .expect("initialized notification should be accepted");
        assert_eq!(service.lifecycle_state(), SessionLifecycle::Running);
    }

    #[tokio::test]
    async fn runtime_tool_registration_enqueues_list_changed_only_when_enabled_and_running() {
        let service = McpServiceImpl::default().with_tools_list_changed(true);

        service
            .register_tool(ToolsRegisterParams {
                name: "runtime".to_string(),
                description: None,
                response_text: "ok".to_string(),
            })
            .await
            .expect("register should succeed");
        assert!(service.drain_notifications().is_empty());

        service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");
        service
            .initialized(Value::Null)
            .await
            .expect("initialized should succeed");

        service
            .register_tool(ToolsRegisterParams {
                name: "runtime-two".to_string(),
                description: Some("runtime".to_string()),
                response_text: "ok".to_string(),
            })
            .await
            .expect("register should succeed");

        let notifications = service.drain_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(
            notifications[0],
            ServerNotification {
                method: "notifications/tools/list_changed".to_string(),
                params: json!({})
            }
        );
    }

    #[tokio::test]
    async fn runtime_tool_registration_rejects_invalid_or_duplicate_names_without_notification() {
        let service = McpServiceImpl::default().with_tools_list_changed(true);

        service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");
        service
            .initialized(Value::Null)
            .await
            .expect("initialized should succeed");

        let empty_name = service
            .register_tool(ToolsRegisterParams {
                name: "   ".to_string(),
                description: None,
                response_text: "ignored".to_string(),
            })
            .await;
        assert!(matches!(empty_name, Err(FittingsError::InvalidParams(_))));
        assert!(service.drain_notifications().is_empty());

        service
            .register_tool(ToolsRegisterParams {
                name: "runtime".to_string(),
                description: None,
                response_text: "ok".to_string(),
            })
            .await
            .expect("first registration should succeed");
        assert_eq!(service.drain_notifications().len(), 1);

        let duplicate = service
            .register_tool(ToolsRegisterParams {
                name: "runtime".to_string(),
                description: None,
                response_text: "another".to_string(),
            })
            .await;
        assert!(matches!(duplicate, Err(FittingsError::InvalidRequest(_))));
        assert!(service.drain_notifications().is_empty());

        let listed = service
            .list_tools(Value::Null)
            .await
            .expect("tools/list should succeed");
        assert_eq!(listed.tools.len(), 1);
        assert_eq!(listed.tools[0].name, "runtime");
    }
}
