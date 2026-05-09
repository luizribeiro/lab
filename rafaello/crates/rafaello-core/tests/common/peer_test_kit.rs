#![allow(dead_code)]
//! Concrete `PeerHandle` test helper for the m2 broker tests
//! (scope §B1, commits c07).
//!
//! `PeerHandle::new` (verified at fittings/crates/core/src/context.rs)
//! takes `(mpsc::Sender<OutboundNotification>, DroppedNotifications,
//! CancellationToken)`. `fresh_peer()` returns the constructed handle
//! together with the notify-channel receiver so tests that want to
//! observe what the broker would have notified can drain it directly.

use fittings_core::context::{DroppedNotifications, OutboundNotification, PeerHandle};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_NOTIFY_CAPACITY: usize = 32;

pub fn fresh_peer() -> (PeerHandle, mpsc::Receiver<OutboundNotification>) {
    fresh_peer_with_capacity(DEFAULT_NOTIFY_CAPACITY)
}

pub fn fresh_peer_with_capacity(
    capacity: usize,
) -> (PeerHandle, mpsc::Receiver<OutboundNotification>) {
    let (tx, rx) = mpsc::channel(capacity);
    let peer = PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new());
    (peer, rx)
}
