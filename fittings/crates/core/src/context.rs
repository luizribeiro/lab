use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{error::FittingsError, message::JsonRpcId};

#[derive(Debug, Clone, PartialEq)]
pub struct OutboundNotification {
    pub method: String,
    pub params: Value,
}

#[derive(Clone)]
pub struct PeerHandle {
    inner: Arc<PeerHandleInner>,
}

struct PeerHandleInner {
    notify_tx: mpsc::UnboundedSender<OutboundNotification>,
}

impl PeerHandle {
    pub fn new(notify_tx: mpsc::UnboundedSender<OutboundNotification>) -> Self {
        Self {
            inner: Arc::new(PeerHandleInner { notify_tx }),
        }
    }

    pub fn notify(&self, method: impl Into<String>, params: Value) -> Result<(), FittingsError> {
        self.inner
            .notify_tx
            .send(OutboundNotification {
                method: method.into(),
                params,
            })
            .map_err(|_| FittingsError::transport("peer notification channel closed"))
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
        let (tx, rx) = mpsc::unbounded_channel();
        // Keep the receiver alive so peer.notify does not surface a Transport
        // error from a closed channel.
        std::mem::forget(rx);
        Self::new(None, CancellationToken::new(), PeerHandle::new(tx))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::{OutboundNotification, PeerHandle, ServiceContext};
    use crate::{error::FittingsError, message::JsonRpcId};

    fn fresh_peer() -> (PeerHandle, mpsc::UnboundedReceiver<OutboundNotification>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (PeerHandle::new(tx), rx)
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
        let (tx, mut rx) = mpsc::unbounded_channel();
        let peer = PeerHandle::new(tx);
        let ctx = ServiceContext::new(None, token, peer);

        ctx.notify("ping", json!({"x": 1}))
            .expect("notify should enqueue");
        let msg = rx.try_recv().expect("receiver should observe message");
        assert_eq!(msg.method, "ping");
        assert_eq!(msg.params, json!({"x": 1}));
    }

    #[test]
    fn peer_notify_returns_transport_when_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel::<OutboundNotification>();
        drop(rx);
        let peer = PeerHandle::new(tx);
        match peer.notify("ping", json!(null)) {
            Err(FittingsError::Transport(_)) => {}
            other => panic!("expected Transport error, got {other:?}"),
        }
    }
}
