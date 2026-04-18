//! DHCP lease preallocation channel used by the gateway's `run`
//! loop. A `LeasePreallocator` is a cheap handle (a cloneable
//! `mpsc::Sender`) that callers use to ask the running gateway for
//! the IP a MAC will receive when its DHCP DISCOVER eventually
//! arrives.
//!
//! Needed because the gateway owns its `DhcpServer` exclusively
//! inside `GatewayStack::run`. Port-forward listeners need the guest
//! IP *before* the guest boots, so we ask the running loop to
//! allocate (or return an existing) lease on demand.

use std::net::Ipv4Addr;

use smoltcp::wire::EthernetAddress;
use tokio::sync::{mpsc, oneshot};

pub(super) struct LeaseRequest {
    pub(super) mac: EthernetAddress,
    pub(super) response: oneshot::Sender<Option<Ipv4Addr>>,
}

/// Handle for preallocating a DHCP lease on a running `GatewayStack`.
///
/// Obtained via [`GatewayStack::lease_preallocator`](super::GatewayStack::lease_preallocator)
/// *before* `run()` consumes the stack. Cheaply cloneable; safe to
/// share across tasks.
#[derive(Clone)]
pub struct LeasePreallocator {
    tx: mpsc::Sender<LeaseRequest>,
}

#[derive(Debug)]
pub enum LeasePreallocationError {
    GatewayGone,
    PoolExhausted,
}

impl std::fmt::Display for LeasePreallocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GatewayGone => f.write_str("gateway is not running"),
            Self::PoolExhausted => f.write_str("DHCP pool exhausted"),
        }
    }
}

impl std::error::Error for LeasePreallocationError {}

impl LeasePreallocator {
    pub(super) fn new(tx: mpsc::Sender<LeaseRequest>) -> Self {
        Self { tx }
    }

    /// Allocate (or return the existing) IP for `mac`. Idempotent:
    /// calling with the same MAC returns the same IP as long as the
    /// lease hasn't been freed.
    pub async fn preallocate(&self, mac: [u8; 6]) -> Result<Ipv4Addr, LeasePreallocationError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(LeaseRequest {
                mac: EthernetAddress(mac),
                response: response_tx,
            })
            .await
            .map_err(|_| LeasePreallocationError::GatewayGone)?;
        response_rx
            .await
            .map_err(|_| LeasePreallocationError::GatewayGone)?
            .ok_or(LeasePreallocationError::PoolExhausted)
    }
}
