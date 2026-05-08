use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use serde_json::Value;
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tokio_util::sync::CancellationToken;

use crate::{error::FittingsError, id_allocator::IdAllocator, message::JsonRpcId};

#[derive(Debug, Clone, PartialEq)]
pub struct OutboundNotification {
    pub method: String,
    pub params: Value,
}

/// Cloneable handle exposing the count of notifications that the local
/// peer dropped because the bounded outbound notification sink was full.
#[derive(Clone, Debug, Default)]
pub struct DroppedNotifications {
    inner: Arc<AtomicU64>,
}

impl DroppedNotifications {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self) -> u64 {
        self.inner.load(Ordering::Relaxed)
    }

    fn record_drop(&self) {
        self.inner.fetch_add(1, Ordering::Relaxed);
    }
}

/// Outbound request emitted by `PeerHandle::call`. The peer-side server loop
/// drains a channel of these and writes them to the wire.
#[derive(Debug)]
pub struct OutboundRequest {
    pub id: JsonRpcId,
    pub method: String,
    pub params: Value,
}

pub type PendingResponse = Result<Value, FittingsError>;

/// In-flight outbound calls keyed by their prefixed id. Entries are inserted
/// by `PeerHandle::call` and resolved by the connection's reader when it
/// observes a matching response.
#[derive(Clone, Default)]
pub struct PendingOutbound {
    inner: Arc<Mutex<HashMap<JsonRpcId, oneshot::Sender<PendingResponse>>>>,
}

impl PendingOutbound {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, id: JsonRpcId, tx: oneshot::Sender<PendingResponse>) {
        self.inner
            .lock()
            .expect("pending map poisoned")
            .insert(id, tx);
    }

    pub fn remove(&self, id: &JsonRpcId) -> Option<oneshot::Sender<PendingResponse>> {
        self.inner.lock().expect("pending map poisoned").remove(id)
    }

    pub fn drain_all(&self) -> Vec<oneshot::Sender<PendingResponse>> {
        self.inner
            .lock()
            .expect("pending map poisoned")
            .drain()
            .map(|(_, tx)| tx)
            .collect()
    }
}

struct OutboundCallChannel {
    id_allocator: Arc<IdAllocator>,
    request_tx: mpsc::UnboundedSender<OutboundRequest>,
    pending: PendingOutbound,
}

#[derive(Clone)]
pub struct PeerHandle {
    inner: Arc<PeerHandleInner>,
}

struct PeerHandleInner {
    notify_tx: mpsc::Sender<OutboundNotification>,
    dropped: DroppedNotifications,
    outbound_call: Option<OutboundCallChannel>,
    closed_token: CancellationToken,
}

impl PeerHandle {
    pub fn new(
        notify_tx: mpsc::Sender<OutboundNotification>,
        dropped: DroppedNotifications,
        closed_token: CancellationToken,
    ) -> Self {
        Self {
            inner: Arc::new(PeerHandleInner {
                notify_tx,
                dropped,
                outbound_call: None,
                closed_token,
            }),
        }
    }

    /// Construct a `PeerHandle` capable of issuing outbound `peer.call`
    /// requests. The caller (typically the connection's serve loop) owns the
    /// receiver end of `request_tx` and the same `pending` map, and is
    /// responsible for routing inbound responses back through the map.
    pub fn with_outbound_calls(
        notify_tx: mpsc::Sender<OutboundNotification>,
        dropped: DroppedNotifications,
        id_allocator: Arc<IdAllocator>,
        request_tx: mpsc::UnboundedSender<OutboundRequest>,
        pending: PendingOutbound,
        closed_token: CancellationToken,
    ) -> Self {
        Self {
            inner: Arc::new(PeerHandleInner {
                notify_tx,
                dropped,
                outbound_call: Some(OutboundCallChannel {
                    id_allocator,
                    request_tx,
                    pending,
                }),
                closed_token,
            }),
        }
    }

    /// Resolves when the underlying transport has torn down (graceful EOF
    /// or transport error). Pending `peer.call` futures are drained with
    /// `FittingsError::Transport` at the same point.
    pub async fn closed(&self) {
        self.inner.closed_token.cancelled().await
    }

    pub async fn call(
        &self,
        method: impl Into<String>,
        params: Value,
    ) -> Result<Value, FittingsError> {
        let outbound = self.inner.outbound_call.as_ref().ok_or_else(|| {
            FittingsError::transport("peer.call not available on this PeerHandle")
        })?;

        let id = outbound.id_allocator.next();
        let (tx, rx) = oneshot::channel();
        outbound.pending.insert(id.clone(), tx);

        let request = OutboundRequest {
            id: id.clone(),
            method: method.into(),
            params,
        };
        if outbound.request_tx.send(request).is_err() {
            outbound.pending.remove(&id);
            return Err(FittingsError::transport("peer call channel closed"));
        }

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(FittingsError::transport("peer call channel closed")),
        }
    }

    pub fn notify(&self, method: impl Into<String>, params: Value) -> Result<(), FittingsError> {
        let method = method.into();
        let notification = OutboundNotification { method, params };
        match self.inner.notify_tx.try_send(notification) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(notification)) => {
                self.inner.dropped.record_drop();
                tracing::warn!(
                    method = %notification.method,
                    "outbound notification dropped: bounded sink full",
                );
                Ok(())
            }
            Err(TrySendError::Closed(_)) => {
                Err(FittingsError::transport("peer notification channel closed"))
            }
        }
    }

    pub fn dropped_notifications(&self) -> DroppedNotifications {
        self.inner.dropped.clone()
    }
}

#[derive(Clone)]
pub struct ServiceContext {
    inner: Arc<ServiceContextInner>,
}

struct ServiceContextInner {
    request_id: Option<JsonRpcId>,
    cancellation_token: CancellationToken,
    peer: PeerHandle,
}

impl ServiceContext {
    pub fn new(
        request_id: Option<JsonRpcId>,
        cancellation_token: CancellationToken,
        peer: PeerHandle,
    ) -> Self {
        Self {
            inner: Arc::new(ServiceContextInner {
                request_id,
                cancellation_token,
                peer,
            }),
        }
    }

    pub fn request_id(&self) -> Option<&JsonRpcId> {
        self.inner.request_id.as_ref()
    }

    pub fn peer(&self) -> &PeerHandle {
        &self.inner.peer
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancellation_token.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.inner.cancellation_token.cancelled().await
    }

    pub fn notify(&self, method: impl Into<String>, params: Value) -> Result<(), FittingsError> {
        self.inner.peer.notify(method, params)
    }

    /// Construct a context with no live peer or cancellation source. Useful for
    /// in-process service invocations (tests, direct calls) where the caller
    /// does not own a server connection. `notify` calls succeed locally but the
    /// notification is discarded.
    pub fn detached() -> Self {
        let (tx, rx) = mpsc::channel(1024);
        // Keep the receiver alive so peer.notify does not surface a Transport
        // error from a closed channel.
        std::mem::forget(rx);
        Self::new(
            None,
            CancellationToken::new(),
            PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new()),
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::{DroppedNotifications, OutboundNotification, PeerHandle, ServiceContext};
    use crate::{error::FittingsError, message::JsonRpcId};

    fn fresh_peer() -> (PeerHandle, mpsc::Receiver<OutboundNotification>) {
        let (tx, rx) = mpsc::channel(16);
        (
            PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new()),
            rx,
        )
    }

    #[tokio::test]
    async fn is_cancelled_tracks_token_state() {
        let token = CancellationToken::new();
        let (peer, _rx) = fresh_peer();
        let ctx = ServiceContext::new(None, token.clone(), peer);

        assert!(!ctx.is_cancelled());
        token.cancel();
        assert!(ctx.is_cancelled());
        ctx.cancelled().await;
    }

    #[tokio::test]
    async fn cancelled_future_resolves_when_token_fires() {
        let token = CancellationToken::new();
        let (peer, _rx) = fresh_peer();
        let ctx = ServiceContext::new(None, token.clone(), peer);

        let waiter_ctx = ctx.clone();
        let waiter = tokio::spawn(async move { waiter_ctx.cancelled().await });

        assert!(!ctx.is_cancelled());
        token.cancel();
        waiter.await.expect("cancellation waiter should complete");
        assert!(ctx.is_cancelled());
    }

    #[test]
    fn request_id_returns_configured_value() {
        let token = CancellationToken::new();
        let (peer, _rx) = fresh_peer();

        let none_ctx = ServiceContext::new(None, token.clone(), peer.clone());
        assert!(none_ctx.request_id().is_none());

        let str_ctx =
            ServiceContext::new(Some(JsonRpcId::from("req-7")), token.clone(), peer.clone());
        assert_eq!(str_ctx.request_id(), Some(&JsonRpcId::from("req-7")));

        let null_ctx = ServiceContext::new(Some(JsonRpcId::Null), token, peer);
        assert_eq!(null_ctx.request_id(), Some(&JsonRpcId::Null));
    }

    #[tokio::test]
    async fn clone_preserves_shared_cancellation_state() {
        let token = CancellationToken::new();
        let (peer, _rx) = fresh_peer();
        let ctx = ServiceContext::new(None, token.clone(), peer);
        let cloned = ctx.clone();

        assert!(!ctx.is_cancelled());
        assert!(!cloned.is_cancelled());

        token.cancel();

        assert!(ctx.is_cancelled());
        assert!(cloned.is_cancelled());
        cloned.cancelled().await;
    }

    #[test]
    fn peer_notify_enqueues_outbound_notification() {
        let token = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(16);
        let peer = PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new());
        let ctx = ServiceContext::new(None, token, peer);

        ctx.notify("ping", json!({"x": 1}))
            .expect("notify should enqueue");
        let msg = rx.try_recv().expect("receiver should observe message");
        assert_eq!(msg.method, "ping");
        assert_eq!(msg.params, json!({"x": 1}));
    }

    #[tokio::test]
    async fn closed_resolves_when_token_fires() {
        let (tx, _rx) = mpsc::channel(1);
        let token = CancellationToken::new();
        let peer = PeerHandle::new(tx, DroppedNotifications::new(), token.clone());
        let waiter = tokio::spawn({
            let peer = peer.clone();
            async move { peer.closed().await }
        });
        token.cancel();
        waiter.await.expect("closed waiter should resolve");
    }

    #[test]
    fn peer_notify_returns_transport_when_channel_closed() {
        let (tx, rx) = mpsc::channel::<OutboundNotification>(1);
        drop(rx);
        let peer = PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new());
        match peer.notify("ping", json!(null)) {
            Err(FittingsError::Transport(_)) => {}
            other => panic!("expected Transport error, got {other:?}"),
        }
    }

    #[test]
    fn peer_notify_drops_when_bounded_sink_full() {
        let (tx, _rx) = mpsc::channel::<OutboundNotification>(1);
        let dropped = DroppedNotifications::new();
        let peer = PeerHandle::new(tx, dropped.clone(), CancellationToken::new());

        peer.notify("first", json!(null)).expect("first fits");
        // Second send fills the slot but is silently dropped (Ok return).
        peer.notify("second", json!(null))
            .expect("drop-on-full reports Ok");
        peer.notify("third", json!(null))
            .expect("drop-on-full reports Ok");

        assert_eq!(dropped.count(), 2);
    }
}
