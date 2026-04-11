use async_trait::async_trait;
use fittings_core::{error::FittingsError, transport::Transport};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, Stdin, Stdout};

pub struct StdioTransport<R, W> {
    reader: BufReader<R>,
    writer: W,
    max_frame_bytes: usize,
}

impl<R, W> StdioTransport<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W, max_frame_bytes: usize) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
            max_frame_bytes,
        }
    }
}

pub fn from_process_stdio(max_frame_bytes: usize) -> StdioTransport<Stdin, Stdout> {
    StdioTransport::new(tokio::io::stdin(), tokio::io::stdout(), max_frame_bytes)
}

#[async_trait]
impl<R, W> Transport for StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
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
                // Overlong frames are terminal transport failures in the MVP.
                // The caller is expected to stop serving after this error.
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
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    use fittings_core::transport::Transport;
    use tokio::io::{duplex, AsyncWrite};

    use super::StdioTransport;

    struct TestWriter {
        written: Vec<u8>,
        flush_calls: usize,
    }

    impl TestWriter {
        fn new() -> Self {
            Self {
                written: Vec::new(),
                flush_calls: 0,
            }
        }
    }

    struct FailingWriteWriter;

    struct FailingFlushWriter;

    impl FailingFlushWriter {
        fn new() -> Self {
            Self
        }
    }

    impl AsyncWrite for TestWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            self.written.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            self.flush_calls += 1;
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for FailingWriteWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            Poll::Ready(Err(std::io::Error::other("write failed")))
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for FailingFlushWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Err(std::io::Error::other("flush failed")))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn recv_reads_one_line_at_a_time() {
        let (mut input_writer, input_reader) = duplex(128);
        let writer = TestWriter::new();
        let mut transport = StdioTransport::new(input_reader, writer, 1024);

        tokio::spawn(async move {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut input_writer, b"first\nsecond\n").await;
        });

        let first = transport.recv().await.expect("first frame should decode");
        let second = transport.recv().await.expect("second frame should decode");

        assert_eq!(first, b"first\n");
        assert_eq!(second, b"second\n");
    }

    #[tokio::test]
    async fn recv_rejects_overlong_frame() {
        let (mut input_writer, input_reader) = duplex(128);
        let writer = TestWriter::new();
        let mut transport = StdioTransport::new(input_reader, writer, 4);

        tokio::spawn(async move {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut input_writer, b"12345\n").await;
        });

        let err = transport
            .recv()
            .await
            .expect_err("frame should be rejected");
        let message = match err {
            fittings_core::error::FittingsError::Transport(message) => message,
            other => panic!("expected transport error, got {other:?}"),
        };

        assert!(message.contains("frame exceeds max_frame_bytes"));
    }

    #[tokio::test]
    async fn recv_returns_end_of_input_on_empty_stream() {
        let (input_writer, input_reader) = duplex(16);
        drop(input_writer);

        let writer = TestWriter::new();
        let mut transport = StdioTransport::new(input_reader, writer, 1024);

        let err = transport.recv().await.expect_err("stream should be at EOF");
        assert!(matches!(
            err,
            fittings_core::error::FittingsError::Transport(message) if message == "end of input"
        ));
    }

    #[tokio::test]
    async fn recv_rejects_partial_frame_at_eof() {
        let (mut input_writer, input_reader) = duplex(16);
        let writer = TestWriter::new();
        let mut transport = StdioTransport::new(input_reader, writer, 1024);

        tokio::spawn(async move {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut input_writer, b"partial").await;
        });

        let err = transport
            .recv()
            .await
            .expect_err("partial frame should fail");
        assert!(matches!(
            err,
            fittings_core::error::FittingsError::Transport(message)
                if message == "unexpected end of input before newline"
        ));
    }

    #[tokio::test]
    async fn send_returns_transport_error_when_write_fails() {
        let (_input_writer, input_reader) = duplex(16);
        let writer = FailingWriteWriter;
        let mut transport = StdioTransport::new(input_reader, writer, 1024);

        let err = transport
            .send(b"{\"id\":\"1\"}\n")
            .await
            .expect_err("write failure should be propagated as a transport error");

        assert!(matches!(
            err,
            fittings_core::error::FittingsError::Transport(message) if message.contains("write failed")
        ));
    }

    #[tokio::test]
    async fn send_returns_transport_error_when_flush_fails() {
        let (_input_writer, input_reader) = duplex(16);
        let writer = FailingFlushWriter::new();
        let mut transport = StdioTransport::new(input_reader, writer, 1024);

        let err = transport
            .send(b"{\"id\":\"1\"}\n")
            .await
            .expect_err("flush failure should be propagated as a transport error");

        assert!(matches!(
            err,
            fittings_core::error::FittingsError::Transport(message) if message.contains("flush failed")
        ));
    }

    #[tokio::test]
    async fn send_writes_frame_and_flushes() {
        let (_input_writer, input_reader) = duplex(16);
        let writer = TestWriter::new();
        let mut transport = StdioTransport::new(input_reader, writer, 1024);

        transport
            .send(b"{\"id\":\"1\"}\n")
            .await
            .expect("send should succeed");

        assert_eq!(transport.writer.written, b"{\"id\":\"1\"}\n");
        assert_eq!(transport.writer.flush_calls, 1);
    }
}
