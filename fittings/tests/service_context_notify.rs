use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{
    fixtures::{parse_response_fixture, request_line},
    memory_transport::MemoryTransport,
};
use serde_json::{json, Value};

const NOTIFY_BURST: usize = 5;

struct NotifyBurstService;

#[async_trait]
impl Service for NotifyBurstService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        for index in 0..NOTIFY_BURST {
            ctx.notify("progress", json!({ "step": index }))
                .expect("notify should enqueue");
        }

        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "done": true }),
            metadata: Default::default(),
        })
    }
}

fn parse_outbound_frame(frame: &[u8]) -> Value {
    serde_json::from_slice(frame).expect("server frame should be valid JSON")
}

#[tokio::test]
async fn handler_notifications_arrive_before_response() {
    let (mut client, server_transport) = MemoryTransport::pair(NOTIFY_BURST + 4);
    let server = Server::new(NotifyBurstService, server_transport);
    let handle = tokio::spawn(server.serve());

    client
        .send(&request_line("ord-1", "work", json!({})))
        .await
        .expect("send request");

    let mut received_steps = Vec::new();
    let mut response = None;
    while response.is_none() {
        let frame = client.recv().await.expect("recv frame");
        let value = parse_outbound_frame(&frame);

        if value.get("id").is_some() {
            let parsed = parse_response_fixture(&frame).expect("parse response");
            response = Some(parsed);
        } else {
            assert_eq!(value["method"], "progress");
            let step = value["params"]["step"]
                .as_u64()
                .expect("notification carries numeric step");
            received_steps.push(step);
        }
    }

    let response = response.expect("response received");
    assert_eq!(response.id, "ord-1");
    assert!(response.error.is_none());

    assert_eq!(received_steps, (0..NOTIFY_BURST as u64).collect::<Vec<_>>());

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
