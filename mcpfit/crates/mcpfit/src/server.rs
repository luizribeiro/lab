//! Top-level server builder.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use fittings::serde_json::{self, Value};
use fittings::tokio;
use fittings::wire::types::JsonRpcId;
use fittings::{
    encode_response_line, to_error_envelope, FittingsError, RequestEnvelope, ResponseEnvelope,
};

use crate::error::McpfitError;
use crate::protocol::ServerInfo;
use crate::registry::ToolRegistry;
use crate::service::{McpServiceImpl, ServerNotification};
use crate::tool::{Tool, ToolSpec};
use crate::Result;

pub const MCP_CANCELLATION_METHOD: &str = "notifications/cancelled";
pub const MCP_CANCELLATION_REQUEST_ID_FIELD: &str = "requestId";

const STDIO_FRAME_LIMIT: usize = 1024 * 1024;

/// Names the JSON-RPC method and `params` field that the transport reads to
/// cancel an in-flight request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationConfig {
    pub method: &'static str,
    pub request_id_field: &'static str,
}

impl CancellationConfig {
    pub const fn mcp() -> Self {
        Self {
            method: MCP_CANCELLATION_METHOD,
            request_id_field: MCP_CANCELLATION_REQUEST_ID_FIELD,
        }
    }
}

impl Default for CancellationConfig {
    fn default() -> Self {
        Self::mcp()
    }
}

/// Builder for an MCP server. Owns the canonical [`ServerInfo`] returned in
/// `initialize` responses and the [`ToolRegistry`] populated via [`Server::tool`].
pub struct Server {
    info: ServerInfo,
    registry: ToolRegistry,
    allow_runtime_registration: bool,
    allow_dynamic_tools: bool,
}

impl Server {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            info: ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            registry: ToolRegistry::new(),
            allow_runtime_registration: false,
            allow_dynamic_tools: false,
        }
    }

    /// Exposes the client-callable `tools/register` JSON-RPC method. The
    /// method still requires a fully initialized session (`Running` lifecycle).
    pub fn allow_runtime_registration(mut self) -> Self {
        self.allow_runtime_registration = true;
        self
    }

    pub fn runtime_registration_allowed(&self) -> bool {
        self.allow_runtime_registration
    }

    /// Advertises `tools.listChanged` without exposing the client-callable
    /// `tools/register` method. Use when the server mutates its own tool list
    /// at runtime (e.g. via embedder-side registration) and clients should
    /// re-fetch on `notifications/tools/list_changed`.
    pub fn allow_dynamic_tools(mut self) -> Self {
        self.allow_dynamic_tools = true;
        self
    }

    pub fn dynamic_tools_allowed(&self) -> bool {
        self.allow_dynamic_tools
    }

    /// Registers a tool. Panics on duplicate names — duplicate registration is
    /// a programmer error in static server construction. Use [`Server::try_tool`]
    /// when registering tools whose names are not known at compile time.
    pub fn tool(self, tool: impl IntoTool) -> Self {
        self.try_tool(tool).expect("tool registration failed")
    }

    /// Returns `Err` on duplicate tool name instead of panicking.
    pub fn try_tool(mut self, tool: impl IntoTool) -> Result<Self> {
        self.registry.register(tool.into_tool())?;
        Ok(self)
    }

    pub fn server_info(&self) -> &ServerInfo {
        &self.info
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Serves the MCP service over the process stdio transport.
    pub async fn serve_stdio(self) -> Result<()> {
        self.serve_with_transport(fittings::from_process_stdio(STDIO_FRAME_LIMIT))
            .await
    }

    /// Process entrypoint kept compatible with existing fittings host
    /// launchers, which set `FITTINGS=1` and pass `serve` as the first arg.
    pub async fn run_entrypoint(self) -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let env_fittings = std::env::var("FITTINGS").ok();
        match classify_entrypoint(env_fittings.as_deref(), &args) {
            EntrypointAction::Serve => self.serve_stdio().await,
            EntrypointAction::PrintUsage => {
                eprintln!(
                    "usage: set FITTINGS=1 and pass `serve` as the first argument \
                     to start the MCP stdio server"
                );
                std::process::exit(2);
            }
        }
    }

    pub(crate) async fn serve_with_transport<T>(self, transport: T) -> Result<()>
    where
        T: fittings::core::transport::Transport + Sync + 'static,
    {
        let service = Arc::new(McpServiceImpl::new(
            self.info,
            self.registry,
            self.allow_runtime_registration,
            self.allow_dynamic_tools,
        ));
        run_mcp_loop(service, transport)
            .await
            .map_err(|err| McpfitError::Internal(err.to_string()))
    }
}

async fn run_mcp_loop<T>(
    service: Arc<McpServiceImpl>,
    mut transport: T,
) -> std::result::Result<(), FittingsError>
where
    T: fittings::core::transport::Transport + Sync + 'static,
{
    let mut workers = tokio::task::JoinSet::<(JsonRpcId, std::result::Result<Value, FittingsError>, Arc<AtomicBool>)>::new();
    let mut in_flight: HashMap<JsonRpcId, Arc<AtomicBool>> = HashMap::new();
    let mut input_closed = false;
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(25));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        if input_closed && workers.is_empty() {
            flush_notifications(&mut transport, &service).await?;
            return Ok(());
        }

        tokio::select! {
            joined = workers.join_next(), if !workers.is_empty() => {
                match joined {
                    Some(Ok((id, result, cancellation))) => {
                        in_flight.remove(&id);
                        flush_notifications(&mut transport, &service).await?;
                        if !cancellation.load(std::sync::atomic::Ordering::Acquire) {
                            let response = match result {
                                Ok(value) => ResponseEnvelope::success(id, value),
                                Err(error) => to_error_envelope(id, error),
                            };
                            send_response(&mut transport, &response).await?;
                        }
                        flush_notifications(&mut transport, &service).await?;
                    }
                    Some(Err(error)) => {
                        return Err(FittingsError::internal(format!(
                            "failed to join request worker: {error}"
                        )));
                    }
                    None => {}
                }
            }
            _ = tick.tick(), if !workers.is_empty() => {
                flush_notifications(&mut transport, &service).await?;
            }
            recv = transport.recv(), if !input_closed => {
                match recv {
                    Ok(frame) => {
                        let request = match fittings::decode_request_line(&frame) {
                            Ok(r) => r,
                            Err(err) => {
                                let (id, ferr) = match err {
                                    fittings::WireDecodeError::Parse(m) => {
                                        (JsonRpcId::Null, FittingsError::parse_error(m))
                                    }
                                    fittings::WireDecodeError::InvalidRequest { message, id } => {
                                        (id.unwrap_or(JsonRpcId::Null), FittingsError::invalid_request(message))
                                    }
                                };
                                send_response(&mut transport, &to_error_envelope(id, ferr)).await?;
                                continue;
                            }
                        };

                        if request.method == "notifications/cancelled" && request.id.is_none() {
                            handle_cancellation(request.params.unwrap_or(Value::Null), &in_flight);
                            continue;
                        }

                        let params = request.params.unwrap_or(Value::Null);
                        let method = request.method;

                        if let Some(id) = request.id {
                            if in_flight.contains_key(&id) {
                                send_response(
                                    &mut transport,
                                    &to_error_envelope(
                                        id,
                                        FittingsError::invalid_request("duplicate in-flight request id"),
                                    ),
                                ).await?;
                                continue;
                            }

                            if method == "tools/call" {
                                let cancellation = Arc::new(AtomicBool::new(false));
                                in_flight.insert(id.clone(), Arc::clone(&cancellation));
                                let svc = Arc::clone(&service);
                                let cancel_for_dispatch = Arc::clone(&cancellation);
                                workers.spawn(async move {
                                    let result = svc
                                        .dispatch(&method, params, Some(cancel_for_dispatch))
                                        .await;
                                    (id, result, cancellation)
                                });
                                continue;
                            }

                            let result = service.dispatch(&method, params, None).await;
                            flush_notifications(&mut transport, &service).await?;
                            let response = match result {
                                Ok(value) => ResponseEnvelope::success(id, value),
                                Err(error) => to_error_envelope(id, error),
                            };
                            send_response(&mut transport, &response).await?;
                            flush_notifications(&mut transport, &service).await?;
                            continue;
                        }

                        let _ = service.dispatch(&method, params, None).await;
                        flush_notifications(&mut transport, &service).await?;
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

fn handle_cancellation(params: Value, in_flight: &HashMap<JsonRpcId, Arc<AtomicBool>>) {
    let Some(obj) = params.as_object() else {
        return;
    };
    let Some(id_value) = obj.get("requestId") else {
        return;
    };
    let id: JsonRpcId = match serde_json::from_value(id_value.clone()) {
        Ok(id) => id,
        Err(_) => return,
    };
    if let Some(token) = in_flight.get(&id) {
        token.store(true, std::sync::atomic::Ordering::Release);
    }
}

async fn flush_notifications<T>(
    transport: &mut T,
    service: &McpServiceImpl,
) -> std::result::Result<(), FittingsError>
where
    T: fittings::core::transport::Transport,
{
    for ServerNotification { method, params } in service.drain_notifications() {
        let envelope = RequestEnvelope::notification(method, Some(params));
        let mut encoded = serde_json::to_vec(&envelope)
            .map_err(|e| FittingsError::internal(format!("encode notification: {e}")))?;
        encoded.push(b'\n');
        transport.send(&encoded).await?;
    }
    Ok(())
}

async fn send_response<T>(
    transport: &mut T,
    response: &ResponseEnvelope,
) -> std::result::Result<(), FittingsError>
where
    T: fittings::core::transport::Transport,
{
    let encoded = encode_response_line(response)
        .map_err(|e| FittingsError::internal(format!("encode response: {e}")))?;
    transport.send(&encoded).await
}

fn is_graceful_eof(error: &FittingsError) -> bool {
    match error {
        FittingsError::Transport(message) => {
            message == "end of input" || message.ends_with("input closed")
        }
        _ => false,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntrypointAction {
    Serve,
    PrintUsage,
}

pub(crate) fn classify_entrypoint(
    env_fittings: Option<&str>,
    args: &[String],
) -> EntrypointAction {
    let first_is_serve = matches!(args.first(), Some(arg) if arg == "serve");
    if env_fittings.is_some() && first_is_serve {
        EntrypointAction::Serve
    } else {
        EntrypointAction::PrintUsage
    }
}

/// Conversion into a [`Tool`] for registration on a [`Server`].
pub trait IntoTool {
    fn into_tool(self) -> Tool;
}

impl IntoTool for Tool {
    fn into_tool(self) -> Tool {
        self
    }
}

impl IntoTool for ToolSpec {
    fn into_tool(self) -> Tool {
        self.build()
    }
}

impl IntoTool for &ToolSpec {
    fn into_tool(self) -> Tool {
        self.build()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_entrypoint, CancellationConfig, EntrypointAction, Server,
        MCP_CANCELLATION_METHOD, MCP_CANCELLATION_REQUEST_ID_FIELD,
    };
    use crate::tool::{Tool, ToolSpec};
    use fittings_testkit::memory_transport::MemoryTransport;

    #[test]
    fn cancellation_constants_match_mcp_wire_strings() {
        assert_eq!(MCP_CANCELLATION_METHOD, "notifications/cancelled");
        assert_eq!(MCP_CANCELLATION_REQUEST_ID_FIELD, "requestId");
        let config = CancellationConfig::default();
        assert_eq!(config.method, MCP_CANCELLATION_METHOD);
        assert_eq!(config.request_id_field, MCP_CANCELLATION_REQUEST_ID_FIELD);
    }

    #[test]
    fn new_records_server_info() {
        let server = Server::new("demo", "1.2.3");
        assert_eq!(server.server_info().name, "demo");
        assert_eq!(server.server_info().version, "1.2.3");
        assert!(server.registry().is_empty());
    }

    #[test]
    fn runtime_registration_disabled_by_default() {
        let server = Server::new("demo", "0.1.0");
        assert!(!server.runtime_registration_allowed());
    }

    #[test]
    fn allow_runtime_registration_sets_flag() {
        let server = Server::new("demo", "0.1.0").allow_runtime_registration();
        assert!(server.runtime_registration_allowed());
    }

    #[test]
    fn dynamic_tools_disabled_by_default() {
        let server = Server::new("demo", "0.1.0");
        assert!(!server.dynamic_tools_allowed());
    }

    #[test]
    fn allow_dynamic_tools_sets_flag_independently() {
        let server = Server::new("demo", "0.1.0").allow_dynamic_tools();
        assert!(server.dynamic_tools_allowed());
        assert!(!server.runtime_registration_allowed());
    }

    #[test]
    fn tool_registers_tool_value() {
        let server = Server::new("demo", "0.1.0").tool(Tool::new("a"));
        assert!(server.registry().contains("a"));
        assert_eq!(server.registry().len(), 1);
    }

    #[test]
    fn tool_registers_tool_spec_by_reference() {
        const SPEC: ToolSpec = ToolSpec::new("from_spec", || Tool::new("from_spec"));
        let server = Server::new("demo", "0.1.0").tool(&SPEC);
        assert!(server.registry().contains("from_spec"));
    }

    #[test]
    fn tool_registers_owned_tool_spec() {
        let spec = ToolSpec::new("owned_spec", || Tool::new("owned_spec"));
        let server = Server::new("demo", "0.1.0").tool(spec);
        assert!(server.registry().contains("owned_spec"));
    }

    #[test]
    fn try_tool_returns_error_on_duplicate() {
        let server = Server::new("demo", "0.1.0").tool(Tool::new("dup"));
        match server.try_tool(Tool::new("dup")) {
            Err(crate::McpfitError::InvalidRequest(_)) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected duplicate registration to fail"),
        }
    }

    #[test]
    #[should_panic(expected = "tool registration failed")]
    fn tool_panics_on_duplicate() {
        let _ = Server::new("demo", "0.1.0")
            .tool(Tool::new("dup"))
            .tool(Tool::new("dup"));
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    #[test]
    fn classify_entrypoint_requires_fittings_env_and_serve_arg() {
        assert_eq!(
            classify_entrypoint(Some("1"), &args(&["serve"])),
            EntrypointAction::Serve
        );
    }

    #[test]
    fn classify_entrypoint_allows_extra_args_after_serve() {
        assert_eq!(
            classify_entrypoint(Some("1"), &args(&["serve", "--debug"])),
            EntrypointAction::Serve
        );
    }

    #[test]
    fn classify_entrypoint_prints_usage_without_fittings_env() {
        assert_eq!(
            classify_entrypoint(None, &args(&["serve"])),
            EntrypointAction::PrintUsage
        );
    }

    #[test]
    fn classify_entrypoint_prints_usage_without_serve_arg() {
        assert_eq!(
            classify_entrypoint(Some("1"), &args(&[])),
            EntrypointAction::PrintUsage
        );
        assert_eq!(
            classify_entrypoint(Some("1"), &args(&["help"])),
            EntrypointAction::PrintUsage
        );
    }

    #[tokio::test]
    async fn serve_with_transport_returns_when_client_disconnects() {
        let (client, server_transport) = MemoryTransport::pair(8);
        let handle = tokio::spawn(
            Server::new("demo", "0.1.0")
                .tool(Tool::new("noop"))
                .serve_with_transport(server_transport),
        );
        drop(client);
        handle
            .await
            .expect("serve task should join")
            .expect("serve should end cleanly when input closes");
    }
}
