use std::{panic::AssertUnwindSafe, sync::Arc};

use std::sync::RwLock;

use fittings_core::{
    context::{
        CancellationConfig, DroppedNotifications, OutboundNotification, OutboundRequest,
        PeerHandle, PendingOutbound, ServiceContext, SharedCancellationConfig,
    },
    error::FittingsError,
    id_allocator::IdAllocator,
    message::{Request, Response},
    service::Service,
    transport::Transport,
};
use fittings_wire::{
    codec::{decode_request_line, encode_response_line, WireDecodeError},
    error_map::{from_error_envelope, to_error_envelope},
    types::{JsonRpcId, RequestEnvelope, ResponseEnvelope},
};
use futures::FutureExt;
use serde_json::Value;
use tokio::{
    sync::{mpsc, Semaphore},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

const DEFAULT_MAX_IN_FLIGHT: usize = 64;
const DEFAULT_NOTIFICATION_CAPACITY: usize = 1024;

pub struct Server<S, T> {
    service: Arc<S>,
    transport: T,
    max_in_flight: usize,
    notification_capacity: usize,
    cancellation: SharedCancellationConfig,
    peer: PeerHandle,
    notify_rx: Option<mpsc::Receiver<OutboundNotification>>,
    dropped_notifications: DroppedNotifications,
    outbound_request_rx: Option<mpsc::UnboundedReceiver<OutboundRequest>>,
    pending_outbound: PendingOutbound,
    id_allocator: Arc<IdAllocator>,
    closed_token: CancellationToken,
}

impl<S, T> Server<S, T>
where
    S: Service + 'static,
    T: Transport + 'static,
{
    pub fn new(service: S, transport: T) -> Self {
        let dropped_notifications = DroppedNotifications::new();
        let (notify_tx, notify_rx) =
            mpsc::channel::<OutboundNotification>(DEFAULT_NOTIFICATION_CAPACITY);
        let (request_tx, request_rx) = mpsc::unbounded_channel::<OutboundRequest>();
        let pending_outbound = PendingOutbound::new();
        let id_allocator = Arc::new(IdAllocator::server());
        let closed_token = CancellationToken::new();
        let cancellation: SharedCancellationConfig =
            Arc::new(RwLock::new(CancellationConfig::lsp_default()));
        let peer = PeerHandle::with_outbound_calls(
            notify_tx,
            dropped_notifications.clone(),
            id_allocator.clone(),
            request_tx,
            pending_outbound.clone(),
            cancellation.clone(),
            closed_token.clone(),
        );
        Self {
            service: Arc::new(service),
            transport,
            max_in_flight: DEFAULT_MAX_IN_FLIGHT,
            notification_capacity: DEFAULT_NOTIFICATION_CAPACITY,
            cancellation,
            peer,
            notify_rx: Some(notify_rx),
            dropped_notifications,
            outbound_request_rx: Some(request_rx),
            pending_outbound,
            id_allocator,
            closed_token,
        }
    }

    pub fn with_max_in_flight(mut self, n: usize) -> Self {
        self.max_in_flight = n.max(1);
        self
    }

    pub fn with_notification_capacity(mut self, n: usize) -> Self {
        self.notification_capacity = n.max(1);
        let (notify_tx, notify_rx) =
            mpsc::channel::<OutboundNotification>(self.notification_capacity);
        let (request_tx, request_rx) = mpsc::unbounded_channel::<OutboundRequest>();
        self.peer = PeerHandle::with_outbound_calls(
            notify_tx,
            self.dropped_notifications.clone(),
            self.id_allocator.clone(),
            request_tx,
            self.pending_outbound.clone(),
            self.cancellation.clone(),
            self.closed_token.clone(),
        );
        self.notify_rx = Some(notify_rx);
        self.outbound_request_rx = Some(request_rx);
        self
    }

    /// Configure the cancellation notification method and id-field extractor
    /// the dispatcher will listen for. The library default is the LSP shape
    /// (`$/cancelRequest`, id field `id`); MCP callers override to
    /// `notifications/cancelled` + `requestId`.
    ///
    /// This commit only stores the configuration; the dispatcher's
    /// token-firing logic that consumes it lands in c21.
    pub fn with_cancellation(self, method: &str, id_field: &str) -> Self {
        {
            let mut cfg = self.cancellation.write().expect("cancellation poisoned");
            cfg.method = method.to_string();
            cfg.id_field = id_field.to_string();
        }
        self
    }

    pub fn cancellation_method(&self) -> String {
        self.cancellation
            .read()
            .expect("cancellation poisoned")
            .method
            .clone()
    }

    pub fn cancellation_id_field(&self) -> String {
        self.cancellation
            .read()
            .expect("cancellation poisoned")
            .id_field
            .clone()
    }

    pub fn dropped_notifications(&self) -> DroppedNotifications {
        self.dropped_notifications.clone()
    }

    /// Connection-scoped peer handle for tasks that live outside any inbound
    /// request handler (e.g. server startup tasks). Inside a handler, the
    /// `PeerHandle` is reachable via `ServiceContext::peer()`.
    pub fn peer(&self) -> PeerHandle {
        self.peer.clone()
    }

    pub async fn serve(mut self) -> Result<(), FittingsError> {
        let semaphore = Arc::new(Semaphore::new(self.max_in_flight));
        let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut notify_rx = self
            .notify_rx
            .take()
            .expect("notify receiver should be present until serve is called");
        let mut outbound_request_rx = self
            .outbound_request_rx
            .take()
            .expect("outbound request receiver should be present until serve is called");
        let pending_outbound = self.pending_outbound.clone();
        let peer = self.peer.clone();
        let mut response_tx = Some(response_tx);
        let mut workers = JoinSet::new();
        let mut accepting = true;

        let result: Result<(), FittingsError> = 'serve: loop {
            if !accepting && workers.is_empty() {
                if let Some(tx) = response_tx.take() {
                    drop(tx);
                }

                let mut drain_error: Option<FittingsError> = None;
                while let Some(frame) = response_rx.recv().await {
                    if let Err(error) = self.transport.send(&frame).await {
                        workers.abort_all();
                        while workers.join_next().await.is_some() {}
                        drain_error = Some(error);
                        break;
                    }
                }
                if let Some(error) = drain_error {
                    break 'serve Err(error);
                }

                while let Ok(notification) = notify_rx.try_recv() {
                    if let Some(frame) = encode_notification(notification) {
                        if let Err(error) = self.transport.send(&frame).await {
                            break 'serve Err(error);
                        }
                    }
                }

                break 'serve Ok(());
            }

            tokio::select! {
                biased;
                Some(notification) = notify_rx.recv() => {
                    if let Some(frame) = encode_notification(notification) {
                        if let Err(error) = self.transport.send(&frame).await {
                            workers.abort_all();
                            while workers.join_next().await.is_some() {}
                            break 'serve Err(error);
                        }
                    }
                }
                Some(request) = outbound_request_rx.recv() => {
                    if let Some(frame) = encode_outbound_request(request) {
                        if let Err(error) = self.transport.send(&frame).await {
                            workers.abort_all();
                            while workers.join_next().await.is_some() {}
                            break 'serve Err(error);
                        }
                    }
                }
                Some(frame) = response_rx.recv() => {
                    if let Err(error) = self.transport.send(&frame).await {
                        workers.abort_all();
                        while workers.join_next().await.is_some() {}
                        break 'serve Err(error);
                    }
                }
                recv_result = self.transport.recv(), if accepting => {
                    match recv_result {
                        Ok(frame) => {
                            if route_inbound_response(&frame, &pending_outbound) {
                                continue;
                            }
                            self.spawn_frame_handler(
                                frame,
                                semaphore.clone(),
                                response_tx.as_ref().expect("response sender should be present").clone(),
                                peer.clone(),
                                &mut workers,
                            ).await;
                        }
                        Err(error) if is_graceful_eof(&error) => {
                            accepting = false;
                        }
                        Err(error) => {
                            workers.abort_all();
                            while workers.join_next().await.is_some() {}
                            break 'serve Err(error);
                        }
                    }
                }
                join_result = workers.join_next(), if !workers.is_empty() => {
                    if let Some(Err(join_error)) = join_result {
                        if join_error.is_panic() {
                            workers.abort_all();
                            while workers.join_next().await.is_some() {}
                            break Err(FittingsError::internal("request worker panicked"));
                        }
                    }
                }
            }
        };

        // Wake any still-pending outbound callers with a transport error so
        // their futures don't hang past the connection's lifetime.
        for pending in pending_outbound.drain_all() {
            let _ = pending.send(Err(FittingsError::transport("peer connection closed")));
        }
        self.closed_token.cancel();

        result
    }

    async fn spawn_frame_handler(
        &self,
        frame: Vec<u8>,
        semaphore: Arc<Semaphore>,
        response_tx: mpsc::UnboundedSender<Vec<u8>>,
        peer: PeerHandle,
        workers: &mut JoinSet<()>,
    ) {
        let permit = match semaphore.acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                return;
            }
        };

        let service = Arc::clone(&self.service);
        workers.spawn(async move {
            let _permit = permit;
            handle_frame(frame, service, response_tx, peer).await;
        });
    }
}

async fn handle_frame<S>(
    frame: Vec<u8>,
    service: Arc<S>,
    response_tx: mpsc::UnboundedSender<Vec<u8>>,
    peer: PeerHandle,
) where
    S: Service + 'static,
{
    let value: Value = match serde_json::from_slice(&frame) {
        Ok(value) => value,
        Err(error) => {
            let response = to_error_envelope(
                JsonRpcId::Null,
                FittingsError::parse_error(error.to_string()),
            );
            send_single_response(response_tx, response);
            return;
        }
    };

    match value {
        Value::Array(batch) => {
            handle_batch_request(batch, service, response_tx, peer).await;
        }
        _ => {
            let response = match decode_request_line(&frame) {
                Ok(request_envelope) => execute_request(request_envelope, service, peer).await,
                Err(error) => {
                    let (id, err) = map_decode_error(error);
                    Some(to_error_envelope(id, err))
                }
            };

            if let Some(response) = response {
                send_single_response(response_tx, response);
            }
        }
    }
}

async fn handle_batch_request<S>(
    batch: Vec<Value>,
    service: Arc<S>,
    response_tx: mpsc::UnboundedSender<Vec<u8>>,
    peer: PeerHandle,
) where
    S: Service + 'static,
{
    if batch.is_empty() {
        let response = to_error_envelope(
            JsonRpcId::Null,
            FittingsError::invalid_request("batch request must not be empty"),
        );
        send_single_response(response_tx, response);
        return;
    }

    let mut responses = Vec::new();

    for item in batch {
        let item_line = match serde_json::to_vec(&item) {
            Ok(item_line) => item_line,
            Err(_) => {
                responses.push(to_error_envelope(
                    JsonRpcId::Null,
                    FittingsError::invalid_request("batch item must be valid JSON-RPC request"),
                ));
                continue;
            }
        };

        let response = match decode_request_line(&item_line) {
            Ok(request_envelope) => {
                execute_request(request_envelope, Arc::clone(&service), peer.clone()).await
            }
            Err(error) => {
                let (id, err) = map_decode_error(error);
                Some(to_error_envelope(id, err))
            }
        };

        if let Some(response) = response {
            responses.push(response);
        }
    }

    if responses.is_empty() {
        return;
    }

    if let Ok(mut encoded) = serde_json::to_vec(&responses) {
        encoded.push(b'\n');
        let _ = response_tx.send(encoded);
    }
}

fn send_single_response(response_tx: mpsc::UnboundedSender<Vec<u8>>, response: ResponseEnvelope) {
    if let Ok(encoded) = encode_response_line(&response) {
        let _ = response_tx.send(encoded);
    }
}

async fn execute_request<S>(
    request: RequestEnvelope,
    service: Arc<S>,
    peer: PeerHandle,
) -> Option<ResponseEnvelope>
where
    S: Service + 'static,
{
    let request_id = request.id.clone();
    let ctx = ServiceContext::new(request_id.clone(), CancellationToken::new(), peer);
    let request = Request {
        id: request.id,
        method: request.method,
        params: request.params.unwrap_or(Value::Null),
        metadata: Default::default(),
    };

    let call_result = AssertUnwindSafe(service.call(request, ctx))
        .catch_unwind()
        .await;

    match (request_id, call_result) {
        (Some(id), Ok(Ok(Response { result, .. }))) => Some(ResponseEnvelope::success(id, result)),
        (Some(id), Ok(Err(error))) => Some(to_error_envelope(id, error)),
        (Some(id), Err(payload)) => Some(to_error_envelope(
            id,
            FittingsError::Panic {
                message: panic_payload_message(payload),
            },
        )),
        (None, _) => None,
    }
}

fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "request handler panicked".to_string()
}

fn encode_notification(notification: OutboundNotification) -> Option<Vec<u8>> {
    let envelope = RequestEnvelope::notification(notification.method, Some(notification.params));
    let mut encoded = serde_json::to_vec(&envelope).ok()?;
    encoded.push(b'\n');
    Some(encoded)
}

fn encode_outbound_request(request: OutboundRequest) -> Option<Vec<u8>> {
    let envelope = RequestEnvelope::new(request.id, request.method, Some(request.params));
    let mut encoded = serde_json::to_vec(&envelope).ok()?;
    encoded.push(b'\n');
    Some(encoded)
}

/// If the inbound frame is a JSON-RPC response (has `result`/`error`) whose
/// id matches a pending outbound call, deliver it to that call and return
/// `true`. Otherwise return `false` so the caller dispatches the frame as a
/// normal inbound request/notification.
fn route_inbound_response(frame: &[u8], pending: &PendingOutbound) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(frame) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if !object.contains_key("result") && !object.contains_key("error") {
        return false;
    }
    let envelope = match fittings_wire::codec::decode_response_line(frame) {
        Ok(envelope) => envelope,
        Err(_) => return false,
    };
    let Some(tx) = pending.remove(&envelope.id) else {
        return false;
    };
    let result = match (envelope.result, envelope.error) {
        (Some(value), None) => Ok(value),
        (None, Some(error)) => Err(from_error_envelope(error)),
        _ => Err(FittingsError::internal(
            "response envelope must contain exactly one of `result` or `error`",
        )),
    };
    let _ = tx.send(result);
    true
}

fn map_decode_error(error: WireDecodeError) -> (JsonRpcId, FittingsError) {
    match error {
        WireDecodeError::Parse(message) => (JsonRpcId::Null, FittingsError::parse_error(message)),
        WireDecodeError::InvalidRequest { message, id } => (
            id.unwrap_or(JsonRpcId::Null),
            FittingsError::invalid_request(message),
        ),
    }
}

fn is_graceful_eof(error: &FittingsError) -> bool {
    match error {
        FittingsError::Transport(message) => {
            message == "end of input" || message.ends_with("input closed")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use async_trait::async_trait;
    use fittings_core::{
        context::ServiceContext,
        error::FittingsError,
        message::{Request, Response},
        service::Service,
        transport::Transport,
    };
    use fittings_testkit::{
        fixtures::{parse_response_fixture, request_line},
        memory_transport::MemoryTransport,
    };
    use fittings_wire::types::{JsonRpcId, ResponseEnvelope};
    use serde_json::{json, Value};
    use tokio::{
        sync::Mutex,
        time::{sleep, timeout, Duration},
    };

    use crate::dispatch::{MethodRouter, RouterService};

    use super::Server;

    fn parse_batch_response_fixture(frame: &[u8]) -> Vec<ResponseEnvelope> {
        let value: Value = serde_json::from_slice(frame).expect("batch frame should be valid JSON");
        let items = value
            .as_array()
            .expect("batch response should be a JSON array");

        items
            .iter()
            .map(|item| {
                let item_line = serde_json::to_vec(item).expect("batch item should serialize");
                parse_response_fixture(&item_line).expect("batch item should decode as response")
            })
            .collect()
    }

    struct DelayService;

    #[async_trait]
    impl Service for DelayService {
        async fn call(
            &self,
            req: Request,
            _ctx: ServiceContext,
        ) -> Result<Response, FittingsError> {
            let delay_ms = req
                .params
                .get("delay_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            sleep(Duration::from_millis(delay_ms)).await;

            Ok(Response {
                id: req.id.unwrap_or(JsonRpcId::Null),
                result: json!({"ok": true}),
                metadata: Default::default(),
            })
        }
    }

    #[test]
    fn cancellation_default_is_lsp_and_override_applies() {
        let (_client, server_transport) = MemoryTransport::pair(1);
        let server = Server::new(DelayService, server_transport);
        assert_eq!(server.cancellation_method(), "$/cancelRequest");
        assert_eq!(server.cancellation_id_field(), "id");

        let (_client, server_transport) = MemoryTransport::pair(1);
        let server = Server::new(DelayService, server_transport)
            .with_cancellation("notifications/cancelled", "requestId");
        assert_eq!(server.cancellation_method(), "notifications/cancelled");
        assert_eq!(server.cancellation_id_field(), "requestId");
    }

    #[tokio::test]
    async fn concurrent_requests_can_complete_out_of_order() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport).with_max_in_flight(8);
        let handle = tokio::spawn(server.serve());

        client
            .send(&request_line("1", "work", json!({"delay_ms": 80})))
            .await
            .expect("send request 1");
        client
            .send(&request_line("2", "work", json!({"delay_ms": 5})))
            .await
            .expect("send request 2");

        let first = parse_response_fixture(&client.recv().await.expect("recv first"))
            .expect("parse first response");
        let second = parse_response_fixture(&client.recv().await.expect("recv second"))
            .expect("parse second response");

        assert_eq!(first.id, "2");
        assert_eq!(second.id, "1");

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn max_in_flight_waits_instead_of_saturating() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport).with_max_in_flight(1);
        let handle = tokio::spawn(server.serve());

        client
            .send(&request_line("1", "work", json!({"delay_ms": 40})))
            .await
            .expect("send request 1");
        client
            .send(&request_line("2", "work", json!({"delay_ms": 0})))
            .await
            .expect("send request 2");

        let first = parse_response_fixture(&client.recv().await.expect("recv first"))
            .expect("parse first response");
        let second = parse_response_fixture(&client.recv().await.expect("recv second"))
            .expect("parse second response");

        assert_eq!(first.id, "1");
        assert_eq!(second.id, "2");

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn notifications_do_not_emit_responses() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(
                br#"{"jsonrpc":"2.0","method":"work","params":{"delay_ms":0}}
"#,
            )
            .await
            .expect("send notification");

        let recv_result = timeout(Duration::from_millis(30), client.recv()).await;
        assert!(
            recv_result.is_err(),
            "notification must not produce a response frame"
        );

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn notification_and_request_can_be_mixed() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(
                br#"{"jsonrpc":"2.0","method":"work","params":{"delay_ms":0}}
"#,
            )
            .await
            .expect("send notification");
        client
            .send(&request_line("mix-1", "work", json!({"delay_ms": 0})))
            .await
            .expect("send request");

        let response = parse_response_fixture(&client.recv().await.expect("recv response"))
            .expect("parse response");
        assert_eq!(response.id, "mix-1");

        let recv_result = timeout(Duration::from_millis(30), client.recv()).await;
        assert!(
            recv_result.is_err(),
            "only the regular request should receive a response"
        );

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn empty_batch_returns_invalid_request_error() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client.send(b"[]\n").await.expect("send empty batch");

        let response = parse_response_fixture(&client.recv().await.expect("recv response"))
            .expect("parse response");
        let error = response.error.expect("response should be an error");

        assert_eq!(response.id, JsonRpcId::Null);
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "batch request must not be empty");

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn batch_with_notifications_and_calls_returns_only_call_responses() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(
                br#"[{"jsonrpc":"2.0","method":"work","params":{"delay_ms":0}},{"jsonrpc":"2.0","id":"batch-1","method":"work","params":{"delay_ms":0}},{"jsonrpc":"2.0","id":"batch-2","method":"work","params":{"delay_ms":0}}]
"#,
            )
            .await
            .expect("send mixed batch");

        let batch_frame = client.recv().await.expect("recv batch response");
        let mut responses = parse_batch_response_fixture(&batch_frame);
        assert_eq!(responses.len(), 2);

        responses.sort_by(|left, right| left.id.to_string().cmp(&right.id.to_string()));
        assert_eq!(responses[0].id, JsonRpcId::from("batch-1"));
        assert_eq!(responses[1].id, JsonRpcId::from("batch-2"));
        assert!(responses.iter().all(|response| response.error.is_none()));

        let recv_result = timeout(Duration::from_millis(30), client.recv()).await;
        assert!(
            recv_result.is_err(),
            "mixed batch should emit exactly one batch response"
        );

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    struct ValidationRouter;

    #[async_trait]
    impl MethodRouter for ValidationRouter {
        async fn route(
            &self,
            method: &str,
            params: serde_json::Value,
            _ctx: ServiceContext,
            _metadata: fittings_core::message::Metadata,
        ) -> Result<serde_json::Value, FittingsError> {
            match method {
                "echo" => {
                    if params.get("value").and_then(|v| v.as_str()).is_none() {
                        return Err(FittingsError::invalid_params("`value` must be a string"));
                    }
                    Ok(json!({"ok": true}))
                }
                _ => Err(FittingsError::method_not_found(method.to_string())),
            }
        }
    }

    #[tokio::test]
    async fn unknown_method_and_invalid_params_map_to_expected_codes() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let service = RouterService::new(ValidationRouter);
        let server = Server::new(service, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(&request_line("not-found", "missing", json!({})))
            .await
            .expect("send missing method request");
        client
            .send(&request_line("bad-params", "echo", json!({"value": 7})))
            .await
            .expect("send invalid params request");

        let first = parse_response_fixture(&client.recv().await.expect("recv first"))
            .expect("parse first response");
        let second = parse_response_fixture(&client.recv().await.expect("recv second"))
            .expect("parse second response");

        let first_error = first.error.expect("first response should be an error");
        let second_error = second.error.expect("second response should be an error");

        assert_eq!(first_error.code, -32601);
        assert_eq!(second_error.code, -32602);

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn decode_errors_distinguish_parse_error_from_invalid_request() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(b"{bad json\n")
            .await
            .expect("send malformed json request");
        client.send(b"[]\n").await.expect("send non-object request");

        let first = parse_response_fixture(&client.recv().await.expect("recv first"))
            .expect("parse first response");
        let second = parse_response_fixture(&client.recv().await.expect("recv second"))
            .expect("parse second response");

        let mut errors = [
            first.error.expect("first response should be an error"),
            second.error.expect("second response should be an error"),
        ];
        errors.sort_by_key(|error| error.code);

        assert_eq!(errors[0].code, -32700);
        assert_ne!(
            errors[0].message, "Parse error",
            "predefined parse message should preserve the typed detail"
        );
        assert!(!errors[0].message.is_empty());
        assert_eq!(errors[1].code, -32600);
        assert_eq!(errors[1].message, "batch request must not be empty");

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_request_reuses_request_id_when_available() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(
                br#"{"jsonrpc":"2.0","id":"bad-1","method":"rpc.ping"}
"#,
            )
            .await
            .expect("send reserved method request");
        client
            .send(
                br#"{"jsonrpc":"2.0","id":"bad-2","method":"ping","extra":true}
"#,
            )
            .await
            .expect("send unknown-field request");

        let first = parse_response_fixture(&client.recv().await.expect("recv first"))
            .expect("parse first response");
        let second = parse_response_fixture(&client.recv().await.expect("recv second"))
            .expect("parse second response");

        let mut responses = vec![first, second];
        responses.sort_by(|left, right| left.id.to_string().cmp(&right.id.to_string()));

        assert_eq!(responses[0].id, JsonRpcId::from("bad-1"));
        assert_eq!(responses[1].id, JsonRpcId::from("bad-2"));

        let mut messages: Vec<String> = Vec::new();
        for response in responses {
            let error = response.error.expect("response should be an error");
            assert_eq!(error.code, -32600);
            assert_ne!(
                error.message, "Invalid Request",
                "predefined invalid-request message should preserve the typed detail"
            );
            assert!(!error.message.is_empty());
            messages.push(error.message);
        }
        assert!(
            messages
                .iter()
                .any(|m| m.contains("method names starting with `rpc.` are reserved")),
            "expected the reserved-method detail among preserved messages: {messages:?}",
        );

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_request_with_unusable_id_returns_null_id() {
        let (mut client, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(DelayService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(
                br#"{"jsonrpc":"2.0","id":{},"method":"ping"}
"#,
            )
            .await
            .expect("send invalid id request");

        let response = parse_response_fixture(&client.recv().await.expect("recv response"))
            .expect("parse response");
        let error = response.error.expect("response should be an error");

        assert_eq!(response.id, JsonRpcId::Null);
        assert_eq!(error.code, -32600);
        assert_ne!(
            error.message, "Invalid Request",
            "predefined invalid-request message should preserve the typed detail"
        );
        assert!(!error.message.is_empty());

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    struct PanicService;

    #[async_trait]
    impl Service for PanicService {
        async fn call(
            &self,
            req: Request,
            _ctx: ServiceContext,
        ) -> Result<Response, FittingsError> {
            if req.method == "boom" {
                panic!("handler panic");
            }

            Ok(Response {
                id: req.id.unwrap_or(JsonRpcId::Null),
                result: json!({"ok": true}),
                metadata: Default::default(),
            })
        }
    }

    #[tokio::test]
    async fn handler_panic_is_converted_to_internal_error_response() {
        let (mut client, server_transport) = MemoryTransport::pair(8);
        let server = Server::new(PanicService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(&request_line("panic-id", "boom", json!({})))
            .await
            .expect("send panic request");

        let response = parse_response_fixture(&client.recv().await.expect("recv response"))
            .expect("parse response");
        let error = response.error.expect("response should be an error");

        assert_eq!(response.id, "panic-id");
        assert_eq!(error.code, -32603);

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    struct WrongIdService;

    #[async_trait]
    impl Service for WrongIdService {
        async fn call(
            &self,
            req: Request,
            _ctx: ServiceContext,
        ) -> Result<Response, FittingsError> {
            let id = req.id.unwrap_or(JsonRpcId::Null);
            Ok(Response {
                id: JsonRpcId::from(format!("{id}-wrong")),
                result: json!({"ok": true}),
                metadata: Default::default(),
            })
        }
    }

    #[tokio::test]
    async fn response_id_always_matches_request_id() {
        let (mut client, server_transport) = MemoryTransport::pair(8);
        let server = Server::new(WrongIdService, server_transport);
        let handle = tokio::spawn(server.serve());

        client
            .send(&request_line("req-123", "ok", json!({})))
            .await
            .expect("send request");

        let response = parse_response_fixture(&client.recv().await.expect("recv response"))
            .expect("parse response");
        assert_eq!(response.id, "req-123");

        drop(client);
        let result = handle.await.expect("server task join");
        assert!(result.is_ok());
    }

    type FrameQueue = Arc<Mutex<VecDeque<Result<Vec<u8>, FittingsError>>>>;

    #[derive(Clone)]
    struct ScriptTransport {
        recv_frames: FrameQueue,
        sent_frames: Arc<Mutex<Vec<Vec<u8>>>>,
        fail_on_send: Arc<AtomicBool>,
    }

    impl ScriptTransport {
        fn new(recv_frames: Vec<Result<Vec<u8>, FittingsError>>) -> Self {
            Self {
                recv_frames: Arc::new(Mutex::new(recv_frames.into_iter().collect())),
                sent_frames: Arc::new(Mutex::new(Vec::new())),
                fail_on_send: Arc::new(AtomicBool::new(false)),
            }
        }

        fn fail_send(self) -> Self {
            self.fail_on_send.store(true, Ordering::SeqCst);
            self
        }

        async fn sent_frames(&self) -> Vec<Vec<u8>> {
            self.sent_frames.lock().await.clone()
        }
    }

    #[async_trait]
    impl Transport for ScriptTransport {
        async fn send(&mut self, frame: &[u8]) -> Result<(), FittingsError> {
            if self.fail_on_send.load(Ordering::SeqCst) {
                return Err(FittingsError::transport("broken pipe"));
            }

            self.sent_frames.lock().await.push(frame.to_vec());
            Ok(())
        }

        async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
            let mut guard = self.recv_frames.lock().await;
            if let Some(next) = guard.pop_front() {
                return next;
            }

            Err(FittingsError::transport("end of input"))
        }
    }

    #[tokio::test]
    async fn eof_drains_in_flight_work_and_exits_cleanly() {
        let request = request_line("drain", "work", json!({"delay_ms": 5}));
        let transport = ScriptTransport::new(vec![
            Ok(request),
            Err(FittingsError::transport("end of input")),
        ]);
        let inspect = transport.clone();
        let server = Server::new(DelayService, transport);

        server.serve().await.expect("serve should end cleanly");

        let sent = inspect.sent_frames().await;
        assert_eq!(sent.len(), 1);
        let response = parse_response_fixture(&sent[0]).expect("parse response");
        assert_eq!(response.id, "drain");
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn writer_failure_causes_global_shutdown_without_hanging() {
        let request_one = request_line("1", "work", json!({"delay_ms": 0}));
        let request_two = request_line("2", "work", json!({"delay_ms": 250}));
        let transport = ScriptTransport::new(vec![Ok(request_one), Ok(request_two)]).fail_send();
        let server = Server::new(DelayService, transport).with_max_in_flight(4);

        let result = timeout(Duration::from_millis(100), server.serve())
            .await
            .expect("server should not hang");

        assert!(matches!(
            result,
            Err(FittingsError::Transport(message)) if message == "broken pipe"
        ));
    }
}
