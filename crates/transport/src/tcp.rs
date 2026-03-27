use async_trait::async_trait;
use fittings_core::{
    error::FittingsError,
    transport::{Connector, Transport},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{tcp::OwnedReadHalf, tcp::OwnedWriteHalf, TcpListener, TcpStream},
};

pub struct TcpTransport {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
    max_frame_bytes: usize,
}

impl TcpTransport {
    pub fn new(stream: TcpStream, max_frame_bytes: usize) -> Self {
        let (reader, writer) = stream.into_split();

        Self {
            reader: BufReader::new(reader),
            writer,
            max_frame_bytes,
        }
    }
}

pub async fn connect_to_address(
    address: &str,
    max_frame_bytes: usize,
) -> Result<TcpTransport, FittingsError> {
    let stream = TcpStream::connect(address)
        .await
        .map_err(|err| FittingsError::transport(err.to_string()))?;

    Ok(TcpTransport::new(stream, max_frame_bytes))
}

pub async fn accept_one(
    listener: &TcpListener,
    max_frame_bytes: usize,
) -> Result<TcpTransport, FittingsError> {
    let (stream, _) = listener
        .accept()
        .await
        .map_err(|err| FittingsError::transport(err.to_string()))?;

    Ok(TcpTransport::new(stream, max_frame_bytes))
}

#[derive(Clone, Debug)]
pub struct TcpConnector {
    address: String,
    max_frame_bytes: usize,
}

impl TcpConnector {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            max_frame_bytes: 1024 * 1024,
        }
    }

    pub fn with_max_frame_bytes(mut self, max_frame_bytes: usize) -> Self {
        self.max_frame_bytes = max_frame_bytes;
        self
    }
}

#[async_trait]
impl Connector for TcpConnector {
    type Connection = TcpTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        connect_to_address(&self.address, self.max_frame_bytes).await
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn send(&mut self, frame: &[u8]) -> Result<(), FittingsError> {
        self.writer
            .write_all(frame)
            .await
            .map_err(|err| FittingsError::transport(err.to_string()))?;
        self.writer
            .flush()
            .await
            .map_err(|err| FittingsError::transport(err.to_string()))
    }

    async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
        let mut frame = Vec::new();

        loop {
            let mut byte = [0_u8; 1];
            let read = self
                .reader
                .read(&mut byte)
                .await
                .map_err(|err| FittingsError::transport(err.to_string()))?;

            if read == 0 {
                if frame.is_empty() {
                    return Err(FittingsError::transport("end of input"));
                }

                return Err(FittingsError::transport(
                    "unexpected end of input before newline",
                ));
            }

            frame.push(byte[0]);

            if frame.len() > self.max_frame_bytes {
                return Err(FittingsError::transport(format!(
                    "frame exceeds max_frame_bytes: {} > {}",
                    frame.len(),
                    self.max_frame_bytes
                )));
            }

            if byte[0] == b'\n' {
                return Ok(frame);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fittings_core::transport::{Connector, Transport};
    use tokio::net::{TcpListener, TcpStream};

    use super::{accept_one, connect_to_address, TcpConnector, TcpTransport};

    #[tokio::test]
    async fn connector_connects_and_roundtrips_frames() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let mut server_transport = accept_one(&listener, 1024).await.expect("accept client");
            let frame = server_transport.recv().await.expect("receive request");
            assert_eq!(frame, b"ping\n");
            server_transport
                .send(b"pong\n")
                .await
                .expect("send response");
        });

        let mut client_transport = TcpConnector::new(address.to_string())
            .with_max_frame_bytes(1024)
            .connect()
            .await
            .expect("connect client");

        client_transport
            .send(b"ping\n")
            .await
            .expect("send request");
        let frame = client_transport.recv().await.expect("receive response");

        assert_eq!(frame, b"pong\n");
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn connect_helper_connects_to_address() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept socket");
            let mut server_transport = TcpTransport::new(stream, 1024);
            server_transport.send(b"hello\n").await.expect("send frame");
        });

        let mut transport = connect_to_address(&address.to_string(), 1024)
            .await
            .expect("connect helper should connect");

        let frame = transport.recv().await.expect("receive frame");
        assert_eq!(frame, b"hello\n");

        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn recv_rejects_overlong_frame() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept socket");
            tokio::io::AsyncWriteExt::write_all(&mut stream, b"12345\n")
                .await
                .expect("write frame");
        });

        let stream = TcpStream::connect(address).await.expect("connect socket");
        let mut transport = TcpTransport::new(stream, 4);

        let err = transport
            .recv()
            .await
            .expect_err("frame should be rejected as overlong");
        let message = match err {
            fittings_core::error::FittingsError::Transport(message) => message,
            other => panic!("expected transport error, got {other:?}"),
        };

        assert!(message.contains("frame exceeds max_frame_bytes"));
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn recv_returns_end_of_input_on_empty_stream() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.expect("accept socket");
        });

        let stream = TcpStream::connect(address).await.expect("connect socket");
        let mut transport = TcpTransport::new(stream, 1024);

        let err = transport.recv().await.expect_err("stream should be at EOF");
        assert!(matches!(
            err,
            fittings_core::error::FittingsError::Transport(message) if message == "end of input"
        ));

        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn recv_rejects_partial_frame_at_eof() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept socket");
            tokio::io::AsyncWriteExt::write_all(&mut stream, b"partial")
                .await
                .expect("write partial frame");
        });

        let stream = TcpStream::connect(address).await.expect("connect socket");
        let mut transport = TcpTransport::new(stream, 1024);

        let err = transport
            .recv()
            .await
            .expect_err("partial frame should fail");
        assert!(matches!(
            err,
            fittings_core::error::FittingsError::Transport(message)
                if message == "unexpected end of input before newline"
        ));

        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn connector_returns_transport_error_when_connection_fails() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        drop(listener);

        let result = TcpConnector::new(address.to_string()).connect().await;
        match result {
            Ok(_) => panic!("connection should fail when no listener is bound"),
            Err(err) => assert!(matches!(
                err,
                fittings_core::error::FittingsError::Transport(_)
            )),
        }
    }
}
