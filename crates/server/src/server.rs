use std::{panic::AssertUnwindSafe, sync::Arc};

use fittings_core::{
    error::FittingsError,
    message::{Request, Response},
    service::Service,
    transport::Transport,
};
use fittings_wire::{
    codec::{decode_request_line, encode_response_line, WireDecodeError},
    error_map::to_error_envelope,
    types::{JsonRpcId, RequestEnvelope, ResponseEnvelope},
};
use futures::FutureExt;
use serde_json::Value;
use tokio::{
    sync::{mpsc, Semaphore},
    task::JoinSet,
};

const DEFAULT_MAX_IN_FLIGHT: usize = 64;

pub struct Server<S, T> {
    service: Arc<S>,
    transport: T,
    max_in_flight: usize,
}

impl<S, T> Server<S, T>
where
    S: Service + 'static,
    T: Transport + 'static,
{
    pub fn new(service: S, transport: T) -> Self {
        Self {
            service: Arc::new(service),
            transport,
            max_in_flight: DEFAULT_MAX_IN_FLIGHT,
        }
    }

    pub fn with_max_in_flight(mut self, n: usize) -> Self {
        self.max_in_flight = n.max(1);
        self
    }

    pub async fn serve(mut self) -> Result<(), FittingsError> {
        let semaphore = Arc::new(Semaphore::new(self.max_in_flight));
        let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut response_tx = Some(response_tx);
        let mut workers = JoinSet::new();
        let mut accepting = true;

        loop {
            if !accepting && workers.is_empty() {
                if let Some(tx) = response_tx.take() {
                    drop(tx);
                }

                while let Some(frame) = response_rx.recv().await {
                    if let Err(error) = self.transport.send(&frame).await {
                        workers.abort_all();
                        while workers.join_next().await.is_some() {}
                        return Err(error);
                    }
                }

                return Ok(());
            }

            tokio::select! {
                Some(frame) = response_rx.recv() => {
                    if let Err(error) = self.transport.send(&frame).await {
                        workers.abort_all();
                        while workers.join_next().await.is_some() {}
                        return Err(error);
                    }
                }
                recv_result = self.transport.recv(), if accepting => {
                    match recv_result {
                        Ok(frame) => {
                            self.spawn_frame_handler(frame, semaphore.clone(), response_tx.as_ref().expect("response sender should be present").clone(), &mut workers).await;
                        }
                        Err(error) if is_graceful_eof(&error) => {
                            accepting = false;
                        }
                        Err(error) => {
                            workers.abort_all();
                            while workers.join_next().await.is_some() {}
                            return Err(error);
                        }
                    }
                }
                join_result = workers.join_next(), if !workers.is_empty() => {
                    if let Some(Err(join_error)) = join_result {
                        if join_error.is_panic() {
                            workers.abort_all();
                            while workers.join_next().await.is_some() {}
                            return Err(FittingsError::internal("request worker panicked"));
                        }
                    }
                }
            }
        }
    }

    async fn spawn_frame_handler(
        &self,
        frame: Vec<u8>,
        semaphore: Arc<Semaphore>,
        response_tx: mpsc::UnboundedSender<Vec<u8>>,
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
            handle_frame(frame, service, response_tx).await;
        });
    }
}

async fn handle_frame<S>(
    frame: Vec<u8>,
    service: Arc<S>,
    response_tx: mpsc::UnboundedSender<Vec<u8>>,
) where
    S: Service + 'static,
{
    let decoded = decode_request_line(&frame);
    let response = match decoded {
        Ok(request_envelope) => execute_request(request_envelope, service).await,
        Err(error) => {
            let err = map_decode_error(error);
            to_error_envelope(JsonRpcId::Null, err)
        }
    };

    if let Ok(encoded) = encode_response_line(&response) {
        let _ = response_tx.send(encoded);
    }
}

async fn execute_request<S>(request: RequestEnvelope, service: Arc<S>) -> ResponseEnvelope
where
    S: Service + 'static,
{
    let request_id = request.id.clone();
    let request = Request {
        id: request.id.to_string(),
        method: request.method,
        params: request.params.unwrap_or(Value::Null),
        metadata: Default::default(),
    };

    let call_result = AssertUnwindSafe(service.call(request)).catch_unwind().await;

    match call_result {
        Ok(Ok(Response { result, .. })) => ResponseEnvelope::success(request_id.clone(), result),
        Ok(Err(error)) => to_error_envelope(request_id.clone(), error),
        Err(_) => to_error_envelope(
            request_id,
            FittingsError::internal("request handler panicked"),
        ),
    }
}

fn map_decode_error(error: WireDecodeError) -> FittingsError {
    match error {
        WireDecodeError::Parse(message) => FittingsError::parse_error(message),
        WireDecodeError::InvalidRequest(message) => FittingsError::invalid_request(message),
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
        error::FittingsError,
        message::{Request, Response},
        service::Service,
        transport::Transport,
    };
    use fittings_testkit::{
        fixtures::{parse_response_fixture, request_line},
        memory_transport::MemoryTransport,
    };
    use serde_json::json;
    use tokio::{
        sync::Mutex,
        time::{sleep, timeout, Duration},
    };

    use crate::dispatch::{MethodRouter, RouterService};

    use super::Server;

    struct DelayService;

    #[async_trait]
    impl Service for DelayService {
        async fn call(&self, req: Request) -> Result<Response, FittingsError> {
            let delay_ms = req
                .params
                .get("delay_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            sleep(Duration::from_millis(delay_ms)).await;

            Ok(Response {
                id: req.id,
                result: json!({"ok": true}),
                metadata: Default::default(),
            })
        }
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

    struct ValidationRouter;

    #[async_trait]
    impl MethodRouter for ValidationRouter {
        async fn route(
            &self,
            method: &str,
            params: serde_json::Value,
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

    struct PanicService;

    #[async_trait]
    impl Service for PanicService {
        async fn call(&self, req: Request) -> Result<Response, FittingsError> {
            if req.method == "boom" {
                panic!("handler panic");
            }

            Ok(Response {
                id: req.id,
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
        async fn call(&self, req: Request) -> Result<Response, FittingsError> {
            Ok(Response {
                id: format!("{}-wrong", req.id),
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

    #[derive(Clone)]
    struct ScriptTransport {
        recv_frames: Arc<Mutex<VecDeque<Result<Vec<u8>, FittingsError>>>>,
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
