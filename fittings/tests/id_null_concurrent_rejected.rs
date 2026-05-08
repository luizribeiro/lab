//! c20 acceptance: a second concurrent inbound `"id": null` request is
//! rejected with `-32600 Invalid Request` per
//! `rfc-fittings-notifications.md:137-145` — at most one explicit
//! null-id request may be in flight at a time. The first request still
//! completes normally once the handler returns.

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::parse_response_fixture, memory_transport::MemoryTransport};
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

struct GatedService {
    started: tokio::sync::Mutex<Option<oneshot::Sender<()>>>,
    release: tokio::sync::Mutex<Option<oneshot::Receiver<()>>>,
}

#[async_trait]
impl Service for GatedService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        if let Some(tx) = self.started.lock().await.take() {
            let _ = tx.send(());
        }
        if let Some(rx) = self.release.lock().await.take() {
            let _ = rx.await;
        }
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({"method": req.method}),
            metadata: Default::default(),
        })
    }
}

#[tokio::test]
async fn second_concurrent_null_id_request_is_rejected() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let service = GatedService {
        started: tokio::sync::Mutex::new(Some(started_tx)),
        release: tokio::sync::Mutex::new(Some(release_rx)),
    };

    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(service, server_transport);
    let server_handle = tokio::spawn(server.serve());

    client
        .send(b"{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"slow\",\"params\":{}}\n")
        .await
        .expect("send first null-id request");

    timeout(Duration::from_millis(500), started_rx)
        .await
        .expect("handler should signal it has started")
        .expect("started channel");

    client
        .send(b"{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"fast\",\"params\":{}}\n")
        .await
        .expect("send second null-id request");

    let frame = timeout(Duration::from_millis(500), client.recv())
        .await
        .expect("duplicate rejection should arrive without blocking on the gated handler")
        .expect("client recv duplicate response");
    let duplicate = parse_response_fixture(&frame).expect("parse response");
    assert_eq!(
        duplicate.id,
        JsonRpcId::Null,
        "duplicate null-id rejection must carry id: null"
    );
    let error = duplicate
        .error
        .expect("duplicate null-id request must surface an error response");
    assert_eq!(error.code, -32600);

    let _ = release_tx.send(());

    let frame = timeout(Duration::from_millis(500), client.recv())
        .await
        .expect("first response should arrive after release")
        .expect("client recv first response");
    let first = parse_response_fixture(&frame).expect("parse response");
    assert_eq!(first.id, JsonRpcId::Null);
    assert!(first.error.is_none());
    assert_eq!(first.result, Some(json!({"method": "slow"})));

    drop(client);
    let _ = server_handle.await.expect("server task should join");
}

#[tokio::test]
async fn null_slot_releases_after_first_request_completes() {
    let (mut client, server_transport) = MemoryTransport::pair(8);

    struct Echo;

    #[async_trait]
    impl Service for Echo {
        async fn call(
            &self,
            req: Request,
            _ctx: ServiceContext,
        ) -> Result<Response, FittingsError> {
            Ok(Response {
                id: req.id.unwrap_or(JsonRpcId::Null),
                result: json!({"method": req.method}),
                metadata: Default::default(),
            })
        }
    }

    let server = Server::new(Echo, server_transport);
    let server_handle = tokio::spawn(server.serve());

    for method in ["one", "two"] {
        let frame = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"{method}\",\"params\":{{}}}}\n"
        );
        client
            .send(frame.as_bytes())
            .await
            .expect("send sequential null-id request");

        let response_frame = timeout(Duration::from_millis(500), client.recv())
            .await
            .expect("response should arrive")
            .expect("client recv");
        let response = parse_response_fixture(&response_frame).expect("parse response");
        assert_eq!(response.id, JsonRpcId::Null);
        assert!(
            response.error.is_none(),
            "sequential null-id requests must each succeed once the prior slot releases"
        );
        assert_eq!(response.result, Some(json!({"method": method})));
    }

    drop(client);
    let _ = server_handle.await.expect("server task should join");
}
