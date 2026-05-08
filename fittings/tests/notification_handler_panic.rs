//! c19 negative acceptance: a panic inside a registered client-side
//! notification handler does not kill subsequent notification delivery and
//! does not affect response correlation for outstanding `Client::call`s.
//! Per scope §K2: handlers run in `tokio::spawn`, which isolates panics.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use fittings::{async_trait::async_trait, Client, Connector, FittingsError, Transport};
use fittings_testkit::fixtures::{parse_request_fixture, success_response_line};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

struct OneShotConnector {
    transport: Arc<Mutex<Option<MemoryTransport>>>,
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
async fn handler_panic_does_not_kill_subsequent_notifications_or_break_calls() {
    let (client_transport, mut server_transport) = MemoryTransport::pair(16);
    let connector = OneShotConnector {
        transport: Arc::new(Mutex::new(Some(client_transport))),
    };

    let observed = Arc::new(AtomicUsize::new(0));
    let panicked = Arc::new(AtomicUsize::new(0));
    let observed_for_handler = Arc::clone(&observed);
    let panicked_for_handler = Arc::clone(&panicked);

    let client = Client::connect(connector)
        .await
        .expect("client connects")
        .with_notification_handler(move |method, _params| {
            observed_for_handler.fetch_add(1, Ordering::SeqCst);
            if method == "boom" {
                panicked_for_handler.fetch_add(1, Ordering::SeqCst);
                panic!("intentional handler panic");
            }
        });

    // Server task: deliver three notifications (panic, normal, normal),
    // then answer one in-flight call to assert correlation still works.
    let server = tokio::spawn(async move {
        for method in ["boom", "after-1", "after-2"] {
            let frame = serde_json::to_vec(&json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": { "k": method }
            }))
            .expect("encode notification");
            let mut frame = frame;
            frame.push(b'\n');
            server_transport
                .send(&frame)
                .await
                .expect("send notification");
        }

        let request = parse_request_fixture(&server_transport.recv().await.expect("request"))
            .expect("decode request");
        let response = success_response_line(
            request.id.as_ref().expect("call carries id"),
            json!({ "ok": true }),
        )
        .expect("encode response");
        server_transport
            .send(&response)
            .await
            .expect("send response");
    });

    // Wait for the three notifications to flow through. We poll the counter
    // rather than sleeping a fixed duration so the test is deterministic.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while observed.load(Ordering::SeqCst) < 3 {
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "expected 3 notifications observed, saw {}",
                observed.load(Ordering::SeqCst)
            );
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(panicked.load(Ordering::SeqCst), 1);

    let result = timeout(Duration::from_millis(500), client.call("ping", json!({})))
        .await
        .expect("call resolves before timeout")
        .expect("call succeeds");
    assert_eq!(result, json!({ "ok": true }));

    server.await.expect("server task join");
}
