//! c14 acceptance: 100 concurrent `peer.call`s in each direction across one
//! connection. All responses correlate. Server-initiated ids carry the `s_`
//! prefix and client-initiated ids carry the `c_` prefix; the two namespaces
//! are disjoint, so the shared in-flight maps cannot collide.

use std::collections::HashSet;

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;
use tokio::time::{timeout, Duration};

const CALLS_PER_SIDE: usize = 100;

struct TagService {
    tag: &'static str,
}

#[async_trait]
impl Service for TagService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        let inbound_id = ctx
            .request_id()
            .and_then(JsonRpcId::as_str)
            .map(str::to_string)
            .unwrap_or_default();
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({
                "tag": self.tag,
                "inbound_id": inbound_id,
                "params": req.params,
            }),
            metadata: Default::default(),
        })
    }
}

struct OneShotConnector {
    transport: tokio::sync::Mutex<Option<MemoryTransport>>,
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
async fn concurrent_bidirectional_calls_isolate_id_namespaces() {
    let (client_transport, server_transport) = MemoryTransport::pair(1024);

    let server = Server::new(TagService { tag: "server" }, server_transport)
        .with_max_in_flight(CALLS_PER_SIDE * 2);
    let server_peer = server.peer();
    let serve = tokio::spawn(server.serve());

    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client connects")
    .with_service(TagService { tag: "client" });
    let client_peer = client.peer();

    let server_calls = (0..CALLS_PER_SIDE).map(|i| {
        let peer = server_peer.clone();
        tokio::spawn(async move { peer.call("from-server", json!({ "i": i })).await })
    });
    let client_calls = (0..CALLS_PER_SIDE).map(|i| {
        let peer = client_peer.clone();
        tokio::spawn(async move { peer.call("from-client", json!({ "i": i })).await })
    });

    let server_handles: Vec<_> = server_calls.collect();
    let client_handles: Vec<_> = client_calls.collect();

    let mut server_results = Vec::with_capacity(CALLS_PER_SIDE);
    for handle in server_handles {
        let r = timeout(Duration::from_secs(5), handle)
            .await
            .expect("server-initiated call resolves");
        server_results.push(r);
    }
    let mut client_results = Vec::with_capacity(CALLS_PER_SIDE);
    for handle in client_handles {
        let r = timeout(Duration::from_secs(5), handle)
            .await
            .expect("client-initiated call resolves");
        client_results.push(r);
    }

    let mut seen_inbound_ids: HashSet<String> = HashSet::new();
    let mut server_seen_is: HashSet<usize> = HashSet::new();
    for join in server_results {
        let result = join
            .expect("server task join")
            .expect("server peer.call ok");
        assert_eq!(result["tag"], "client", "answered by client side");
        let inbound = result["inbound_id"]
            .as_str()
            .expect("inbound id")
            .to_string();
        assert!(
            inbound.starts_with("s_"),
            "server-initiated peer.call must carry s_ prefix: {inbound}",
        );
        assert!(seen_inbound_ids.insert(inbound), "ids must be unique");
        let i = result["params"]["i"].as_u64().expect("i") as usize;
        assert!(server_seen_is.insert(i), "no duplicate i");
    }
    assert_eq!(server_seen_is.len(), CALLS_PER_SIDE);

    let mut client_seen_is: HashSet<usize> = HashSet::new();
    for join in client_results {
        let result = join
            .expect("client task join")
            .expect("client peer.call ok");
        assert_eq!(result["tag"], "server", "answered by server side");
        let inbound = result["inbound_id"]
            .as_str()
            .expect("inbound id")
            .to_string();
        assert!(
            inbound.starts_with("c_"),
            "client-initiated peer.call must carry c_ prefix: {inbound}",
        );
        assert!(seen_inbound_ids.insert(inbound), "ids must be unique");
        let i = result["params"]["i"].as_u64().expect("i") as usize;
        assert!(client_seen_is.insert(i), "no duplicate i");
    }
    assert_eq!(client_seen_is.len(), CALLS_PER_SIDE);

    assert_eq!(
        seen_inbound_ids.len(),
        CALLS_PER_SIDE * 2,
        "every id observed across both directions must be unique",
    );

    drop(client);
    let serve_result = timeout(Duration::from_secs(5), serve)
        .await
        .expect("serve exits")
        .expect("server task join");
    assert!(
        serve_result.is_ok(),
        "server should exit cleanly: {serve_result:?}"
    );
}
