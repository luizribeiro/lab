use std::sync::{Arc, Mutex};

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;

struct Observation {
    request_id: Option<JsonRpcId>,
    is_cancelled: bool,
}

struct ObservingService {
    observed: Arc<Mutex<Option<Observation>>>,
}

#[async_trait]
impl Service for ObservingService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        let observation = Observation {
            request_id: ctx.request_id().cloned(),
            is_cancelled: ctx.is_cancelled(),
        };
        *self.observed.lock().expect("observed mutex") = Some(observation);

        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({"ok": true}),
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
async fn handler_observes_request_id_and_initial_cancellation_state() {
    let observed = Arc::new(Mutex::new(None));
    let service = ObservingService {
        observed: Arc::clone(&observed),
    };

    let (client_transport, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(service, server_transport);
    let server_handle = tokio::spawn(server.serve());

    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client should connect");

    let result = client
        .call("ping", json!({}))
        .await
        .expect("call should succeed");
    assert_eq!(result, json!({"ok": true}));

    let observation = observed
        .lock()
        .expect("observed mutex")
        .take()
        .expect("handler should have recorded observation");
    assert!(
        matches!(observation.request_id, Some(JsonRpcId::String(_))),
        "request id should be present and string-typed for the dispatcher path",
    );
    assert!(
        !observation.is_cancelled,
        "freshly-issued request must not be cancelled at handler entry"
    );

    drop(client);
    let _ = server_handle.await.expect("server task should join");
}
