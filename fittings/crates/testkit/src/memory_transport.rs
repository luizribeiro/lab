use async_trait::async_trait;
use fittings_core::{error::FittingsError, transport::Transport};
use tokio::sync::mpsc;

pub struct MemoryTransport {
    incoming: mpsc::Receiver<Vec<u8>>,
    outgoing: mpsc::Sender<Vec<u8>>,
}

impl MemoryTransport {
    pub fn pair(buffer: usize) -> (Self, Self) {
        let (a_tx, a_rx) = mpsc::channel(buffer);
        let (b_tx, b_rx) = mpsc::channel(buffer);

        let left = Self {
            incoming: a_rx,
            outgoing: b_tx,
        };
        let right = Self {
            incoming: b_rx,
            outgoing: a_tx,
        };

        (left, right)
    }
}

#[async_trait]
impl Transport for MemoryTransport {
    async fn send(&mut self, frame: &[u8]) -> Result<(), FittingsError> {
        self.outgoing
            .send(frame.to_vec())
            .await
            .map_err(|_| FittingsError::transport("memory transport output closed"))
    }

    async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
        self.incoming
            .recv()
            .await
            .ok_or_else(|| FittingsError::transport("memory transport input closed"))
    }
}

#[cfg(test)]
mod tests {
    use fittings_core::{error::FittingsError, transport::Transport};

    use super::MemoryTransport;

    #[tokio::test]
    async fn pair_delivers_frames_in_order() {
        let (mut left, mut right) = MemoryTransport::pair(8);

        left.send(b"one\n").await.expect("send one");
        left.send(b"two\n").await.expect("send two");
        left.send(b"three\n").await.expect("send three");

        assert_eq!(right.recv().await.expect("recv one"), b"one\n");
        assert_eq!(right.recv().await.expect("recv two"), b"two\n");
        assert_eq!(right.recv().await.expect("recv three"), b"three\n");
    }

    #[tokio::test]
    async fn pair_is_duplex() {
        let (mut left, mut right) = MemoryTransport::pair(8);

        left.send(b"left->right\n").await.expect("left sends");
        right.send(b"right->left\n").await.expect("right sends");

        assert_eq!(
            right.recv().await.expect("right receives"),
            b"left->right\n"
        );
        assert_eq!(left.recv().await.expect("left receives"), b"right->left\n");
    }

    #[tokio::test]
    async fn send_returns_transport_error_when_peer_is_dropped() {
        let (mut left, right) = MemoryTransport::pair(1);
        drop(right);

        let err = left.send(b"ping\n").await.expect_err("send should fail");
        assert!(matches!(
            err,
            FittingsError::Transport(message) if message == "memory transport output closed"
        ));
    }

    #[tokio::test]
    async fn recv_returns_transport_error_when_peer_is_dropped() {
        let (left, mut right) = MemoryTransport::pair(1);
        drop(left);

        let err = right.recv().await.expect_err("recv should fail");
        assert!(matches!(
            err,
            FittingsError::Transport(message) if message == "memory transport input closed"
        ));
    }
}
