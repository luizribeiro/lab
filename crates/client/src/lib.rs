use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
};

use fittings_core::{error::FittingsError, transport::Connector};
use fittings_wire::{
    error_map::from_error_envelope,
    types::{RequestEnvelope, ResponseEnvelope},
};
use serde_json::Value;
use tokio::{
    sync::{mpsc, mpsc::error::TryRecvError, oneshot},
    task::JoinHandle,
};

pub struct Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    request_tx: mpsc::UnboundedSender<ClientCommand>,
    next_id: AtomicU64,
    worker: JoinHandle<()>,
    _connector: PhantomData<C>,
}

impl<C> Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    pub async fn connect(connector: C) -> Result<Self, FittingsError> {
        let transport = connector.connect().await?;
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let worker = tokio::spawn(run_client_loop(transport, request_rx));

        Ok(Self {
            request_tx,
            next_id: AtomicU64::new(1),
            worker,
            _connector: PhantomData,
        })
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, FittingsError> {
        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(ClientCommand::Call {
                id,
                method: method.to_string(),
                params,
                response_tx,
            })
            .map_err(|_| FittingsError::internal("client is not connected"))?;

        response_rx
            .await
            .map_err(|_| FittingsError::internal("client request canceled"))?
    }

    fn next_request_id(&self) -> String {
        self.next_id.fetch_add(1, Ordering::Relaxed).to_string()
    }
}

impl<C> Drop for Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    fn drop(&mut self) {
        let _ = self.request_tx.send(ClientCommand::Shutdown);
        self.worker.abort();
    }
}

enum ClientCommand {
    Call {
        id: String,
        method: String,
        params: Value,
        response_tx: oneshot::Sender<Result<Value, FittingsError>>,
    },
    Shutdown,
}

async fn run_client_loop<T>(
    mut transport: T,
    mut request_rx: mpsc::UnboundedReceiver<ClientCommand>,
) where
    T: fittings_core::transport::Transport + Send + 'static,
{
    let mut pending: HashMap<String, oneshot::Sender<Result<Value, FittingsError>>> =
        HashMap::new();

    loop {
        pending.retain(|_, sender| !sender.is_closed());

        tokio::select! {
            command = request_rx.recv() => {
                match command {
                    Some(ClientCommand::Call { id, method, params, response_tx }) => {
                        if let Err(error) = send_request(&mut transport, &id, &method, params).await {
                            let _ = response_tx.send(Err(error.clone()));
                            fail_pending(&mut pending, error.clone());
                            fail_queued_calls(&mut request_rx, error);
                            return;
                        }

                        pending.insert(id, response_tx);
                    }
                    Some(ClientCommand::Shutdown) | None => {
                        fail_pending(&mut pending, FittingsError::internal("client closed"));
                        return;
                    }
                }
            }
            recv_result = transport.recv(), if !pending.is_empty() => {
                match recv_result {
                    Ok(frame) => handle_response_frame(frame, &mut pending),
                    Err(error) => {
                        fail_pending(&mut pending, error.clone());
                        fail_queued_calls(&mut request_rx, error);
                        return;
                    }
                }
            }
        }
    }
}

async fn send_request<T>(
    transport: &mut T,
    id: &str,
    method: &str,
    params: Value,
) -> Result<(), FittingsError>
where
    T: fittings_core::transport::Transport,
{
    let request = RequestEnvelope {
        id: id.to_string(),
        method: method.to_string(),
        params,
        metadata: Default::default(),
    };

    let mut encoded = serde_json::to_vec(&request)
        .map_err(|error| FittingsError::internal(format!("failed to encode request: {error}")))?;
    encoded.push(b'\n');

    transport.send(&encoded).await
}

fn handle_response_frame(
    frame: Vec<u8>,
    pending: &mut HashMap<String, oneshot::Sender<Result<Value, FittingsError>>>,
) {
    let envelope: ResponseEnvelope = match serde_json::from_slice(&frame) {
        Ok(envelope) => envelope,
        Err(error) => {
            fail_pending(
                pending,
                FittingsError::invalid_request(format!(
                    "failed to decode response envelope: {error}"
                )),
            );
            return;
        }
    };

    let Some(response_tx) = pending.remove(&envelope.id) else {
        return;
    };

    let result = match (envelope.result, envelope.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(from_error_envelope(error)),
        _ => Err(FittingsError::invalid_request(
            "response must contain exactly one of `result` or `error`",
        )),
    };

    let _ = response_tx.send(result);
}

fn fail_pending(
    pending: &mut HashMap<String, oneshot::Sender<Result<Value, FittingsError>>>,
    error: FittingsError,
) {
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(error.clone()));
    }
}

fn fail_queued_calls(
    request_rx: &mut mpsc::UnboundedReceiver<ClientCommand>,
    error: FittingsError,
) {
    loop {
        match request_rx.try_recv() {
            Ok(ClientCommand::Call { response_tx, .. }) => {
                let _ = response_tx.send(Err(error.clone()));
            }
            Ok(ClientCommand::Shutdown) => {}
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use fittings_core::{
        error::FittingsError,
        transport::{Connector, Transport},
    };
    use fittings_testkit::fixtures::{
        error_response_line, parse_request_fixture, success_response_line,
    };
    use fittings_testkit::memory_transport::MemoryTransport;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::Client;

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

    struct FailingSendTransport;

    #[async_trait]
    impl Transport for FailingSendTransport {
        async fn send(&mut self, _frame: &[u8]) -> Result<(), FittingsError> {
            Err(FittingsError::transport("simulated write failure"))
        }

        async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
            Err(FittingsError::transport("simulated recv failure"))
        }
    }

    struct FailingConnector;

    #[async_trait]
    impl Connector for FailingConnector {
        type Connection = FailingSendTransport;

        async fn connect(&self) -> Result<Self::Connection, FittingsError> {
            Ok(FailingSendTransport)
        }
    }

    #[tokio::test]
    async fn correlates_out_of_order_responses_by_request_id() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(16);
        let client = Client::connect(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect");

        let server = tokio::spawn(async move {
            let first = parse_request_fixture(&server_transport.recv().await.expect("request one"))
                .expect("decode request one");
            let second =
                parse_request_fixture(&server_transport.recv().await.expect("request two"))
                    .expect("decode request two");

            let second_response =
                success_response_line(&second.id, json!({"method": second.method, "order": 2}))
                    .expect("encode second response");
            server_transport
                .send(&second_response)
                .await
                .expect("send second response");

            let first_response =
                success_response_line(&first.id, json!({"method": first.method, "order": 1}))
                    .expect("encode first response");
            server_transport
                .send(&first_response)
                .await
                .expect("send first response");
        });

        let call_one = client.call("slow", json!({"n": 1}));
        let call_two = client.call("fast", json!({"n": 2}));
        let (result_one, result_two) = tokio::join!(call_one, call_two);

        assert_eq!(
            result_one.expect("call one result"),
            json!({"method": "slow", "order": 1})
        );
        assert_eq!(
            result_two.expect("call two result"),
            json!({"method": "fast", "order": 2})
        );

        server.await.expect("server task should join");
    }

    #[tokio::test]
    async fn timeout_cancellation_does_not_break_subsequent_calls() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(16);
        let client = Client::connect(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect");

        let server = tokio::spawn(async move {
            let first = parse_request_fixture(&server_transport.recv().await.expect("request one"))
                .expect("decode request one");

            tokio::time::sleep(Duration::from_millis(60)).await;
            let first_response = success_response_line(&first.id, json!({"ok": true}))
                .expect("encode first response");
            server_transport
                .send(&first_response)
                .await
                .expect("send first response");

            let second =
                parse_request_fixture(&server_transport.recv().await.expect("request two"))
                    .expect("decode request two");
            let second_response = success_response_line(&second.id, json!({"ok": true, "call": 2}))
                .expect("encode second response");
            server_transport
                .send(&second_response)
                .await
                .expect("send second response");
        });

        let timed_out =
            tokio::time::timeout(Duration::from_millis(20), client.call("slow", json!({}))).await;
        assert!(
            timed_out.is_err(),
            "first call should time out and be canceled"
        );

        let second = client
            .call("fast", json!({}))
            .await
            .expect("second call should succeed after first is canceled");
        assert_eq!(second, json!({"ok": true, "call": 2}));

        server.await.expect("server task should join");
    }

    #[tokio::test]
    async fn transport_close_fails_pending_calls_deterministically() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(16);
        let client = Client::connect(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect");

        let server = tokio::spawn(async move {
            let _ = server_transport.recv().await.expect("receive request");
            drop(server_transport);
        });

        let error = client
            .call("will-fail", json!({}))
            .await
            .expect_err("call should fail when transport closes");

        assert!(matches!(
            error,
            FittingsError::Transport(message) if message == "memory transport input closed"
        ));

        server.await.expect("server task should join");
    }

    #[tokio::test]
    async fn call_maps_error_envelopes_using_reverse_error_mapping() {
        let (client_transport, mut server_transport) = MemoryTransport::pair(16);
        let client = Client::connect(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect");

        let server = tokio::spawn(async move {
            let request = parse_request_fixture(&server_transport.recv().await.expect("request"))
                .expect("decode request");
            let response = error_response_line(&request.id, -32601, "missing-method")
                .expect("encode error response");
            server_transport
                .send(&response)
                .await
                .expect("send error response");
        });

        let error = client
            .call("missing-method", json!({}))
            .await
            .expect_err("call should return mapped method-not-found error");

        assert!(matches!(
            error,
            FittingsError::MethodNotFound(message) if message == "missing-method"
        ));

        server.await.expect("server task should join");
    }

    #[tokio::test]
    async fn fatal_send_error_is_propagated_to_queued_calls() {
        let client = Client::connect(FailingConnector)
            .await
            .expect("client should connect");

        let call_one = client.call("a", json!({}));
        let call_two = client.call("b", json!({}));
        let call_three = client.call("c", json!({}));

        let (result_one, result_two, result_three) = tokio::join!(call_one, call_two, call_three);

        for result in [result_one, result_two, result_three] {
            let error = result.expect_err("all calls should fail with transport error");
            assert!(matches!(
                error,
                FittingsError::Transport(message) if message == "simulated write failure"
            ));
        }
    }
}
