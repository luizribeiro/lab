//! c25 acceptance: peer-gone is observed via `peer.closed()` and pending
//! `peer.call` futures resolving with `FittingsError::Transport`, not via
//! a synchronous `Cancelled` from `ctx.notify`.
//!
//! Per `rfc-fittings-notifications.md:717-747`, `notify` reports only
//! local enqueue/encoding/channel-closed status. Peer disconnect mid-
//! stream is asynchronous from the handler's perspective: the
//! dispatcher discovers the close on its next transport write, resolves
//! `peer.closed()`, and drains pending outbound calls with `Transport`.
//! This test pins that contract.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::request_line, memory_transport::MemoryTransport};
use serde_json::json;
use tokio::{sync::Notify, time::timeout};

struct NotifyForeverService {
    started: Arc<Notify>,
    notified: Arc<AtomicUsize>,
    saw_cancelled: Arc<AtomicBool>,
}

#[async_trait]
impl Service for NotifyForeverService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        self.started.notify_waiters();

        loop {
            match ctx.notify(
                "progress",
                json!({ "i": self.notified.load(Ordering::SeqCst) }),
            ) {
                Ok(()) => {
                    self.notified.fetch_add(1, Ordering::SeqCst);
                }
                Err(FittingsError::Cancelled { .. }) => {
                    self.saw_cancelled.store(true, Ordering::SeqCst);
                    break;
                }
                Err(_) => {
                    // Transport (channel closed on dispatcher shutdown) or
                    // Internal (encode failure) are both compatible with the
                    // RFC contract — bail and let the test assert the
                    // observed-via-peer.closed signal.
                    break;
                }
            }

            tokio::select! {
                _ = ctx.cancelled() => {
                    // Per RFC: handlers may still call notify after
                    // cancellation resolves. The contract is that notify
                    // reports local channel status, never Cancelled.
                    if let Err(FittingsError::Cancelled { .. }) =
                        ctx.notify("progress", json!({ "after_cancel": true }))
                    {
                        self.saw_cancelled.store(true, Ordering::SeqCst);
                    }
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(2)) => {}
            }
        }

        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "done": true }),
            metadata: Default::default(),
        })
    }
}

#[tokio::test]
async fn peer_gone_observed_via_closed_and_transport_not_via_notify_cancelled() {
    let started = Arc::new(Notify::new());
    let notified = Arc::new(AtomicUsize::new(0));
    let saw_cancelled = Arc::new(AtomicBool::new(false));

    let service = NotifyForeverService {
        started: started.clone(),
        notified: notified.clone(),
        saw_cancelled: saw_cancelled.clone(),
    };

    let (mut raw_client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(service, server_transport);
    let peer = server.peer();
    let serve = tokio::spawn(server.serve());

    // Kick the handler.
    raw_client
        .send(&request_line("req-1", "stream", json!({})))
        .await
        .expect("send request");

    timeout(Duration::from_millis(500), started.notified())
        .await
        .expect("handler should start");

    // Open a server-initiated peer.call that will never be answered: the
    // raw client never sends a response. We will assert it drains to
    // Transport once the peer goes away.
    let pending = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("ask", json!({})).await })
    };

    // Drain a few notification frames so we're certain the handler is
    // mid-stream when the peer disappears.
    for _ in 0..3 {
        let frame = timeout(Duration::from_millis(500), raw_client.recv())
            .await
            .expect("frame within timeout")
            .expect("recv frame");
        let value: serde_json::Value = serde_json::from_slice(&frame).expect("frame is valid JSON");
        // We expect notification frames or a peer.call request frame; both
        // confirm the dispatcher is alive.
        assert!(value.get("method").is_some(), "frame should carry a method");
    }

    // Peer goes away.
    drop(raw_client);

    // Contract bullet 1: peer.closed() resolves.
    timeout(Duration::from_millis(1_000), peer.closed())
        .await
        .expect("peer.closed() must resolve after transport tear-down");

    // Contract bullet 2: pending peer.call resolves with Transport.
    let pending_result = timeout(Duration::from_millis(1_000), pending)
        .await
        .expect("pending peer.call must resolve after peer goes away")
        .expect("peer.call task join");
    assert!(
        matches!(pending_result, Err(FittingsError::Transport(_))),
        "pending peer.call must drain to Transport, got {pending_result:?}",
    );

    // Give the handler a window to keep calling notify after the peer
    // disappears. The contract is that notify never returns Cancelled even
    // in this regime — drop-on-full / channel-closed only.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Contract bullet 3: notify never reports peer-gone synchronously.
    assert!(
        !saw_cancelled.load(Ordering::SeqCst),
        "ctx.notify must NOT return Cancelled on peer-gone — \
         peer-gone is observed via peer.closed()/Transport (RFC \
         rfc-fittings-notifications.md:717-747)",
    );

    // The handler may still be running its notify loop, but the dispatcher
    // is on its way out. Aborting the serve task tears down any remaining
    // handler tasks so the test exits cleanly.
    serve.abort();
    let _ = timeout(Duration::from_millis(500), serve).await;
}
