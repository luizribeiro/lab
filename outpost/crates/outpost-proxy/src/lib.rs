//! Loopback HTTP CONNECT proxy that enforces an [`outpost::NetworkPolicy`]
//! against CONNECT target authorities.
//!
//! Lockin's `proxy` sandbox network mode spawns this before launching
//! the child, injects `HTTP_PROXY`/`HTTPS_PROXY` into the child env,
//! and denies non-loopback outbound at the OS sandbox layer. Apps
//! honoring the proxy env see their policy enforced per-hostname;
//! apps that ignore it fail closed at the sandbox.
//!
//! TLS is tunneled end-to-end (no MITM): the proxy only inspects the
//! CONNECT target authority. Policy verdict is based on the hostname
//! string the client put on the wire.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use outpost::{NetworkPolicy, PolicyAction};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Maximum bytes allowed in the request head before the client's
/// `\r\n\r\n` delimiter. Well above any realistic CONNECT + Host
/// header combination.
const MAX_REQUEST_HEAD_BYTES: usize = 8 * 1024;

/// Upper bound on how long we wait for the client to finish sending
/// its request head. Bounds slow-loris from a misbehaving sandboxed
/// child.
#[cfg(not(test))]
const REQUEST_HEAD_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const REQUEST_HEAD_TIMEOUT: Duration = Duration::from_millis(250);

/// Handle to a running proxy. Dropping the handle shuts the proxy
/// down: a shutdown signal is sent to the accept loop and the
/// background task is aborted. The kernel listener is closed
/// asynchronously when the task unwinds, so a brief window may
/// remain where `connect()` still succeeds against the port; this is
/// acceptable for the intended use (owned by the sandbox-child
/// lifecycle in lockin — the child is already dying when this drops).
pub struct ProxyHandle {
    listen_addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl ProxyHandle {
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }
}

impl Drop for ProxyHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Bind a loopback listener on an ephemeral port and start the
/// accept loop. The returned [`ProxyHandle`] is immediately usable —
/// the listener is bound before this function returns.
///
/// Rejects policies with `default_action == PolicyAction::Log`: in a
/// proxy context that produces silent deny-all, which is more likely
/// a misconfiguration than a deliberate choice, so we surface it
/// loudly instead.
pub async fn start(policy: NetworkPolicy) -> io::Result<ProxyHandle> {
    if matches!(policy.default_action, PolicyAction::Log) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "policy default_action cannot be Log for proxy enforcement",
        ));
    }

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let listen_addr = listener.local_addr()?;
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let policy = Arc::new(policy);

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, peer)) => {
                            let policy = Arc::clone(&policy);
                            tokio::spawn(async move {
                                if let Err(err) = handle_connection(stream, policy).await {
                                    tracing::debug!(
                                        %peer,
                                        error = %err,
                                        "proxy connection closed with error"
                                    );
                                }
                            });
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "proxy accept failed");
                        }
                    }
                }
            }
        }
    });

    Ok(ProxyHandle {
        listen_addr,
        shutdown_tx: Some(shutdown_tx),
        task: Some(task),
    })
}

async fn handle_connection(mut stream: TcpStream, policy: Arc<NetworkPolicy>) -> io::Result<()> {
    let request =
        match tokio::time::timeout(REQUEST_HEAD_TIMEOUT, read_request_head(&mut stream)).await {
            Ok(r) => r?,
            Err(_) => {
                write_status(&mut stream, 408, "Request Timeout").await?;
                return Ok(());
            }
        };

    let Some(authority) = parse_connect_authority(&request) else {
        write_status(&mut stream, 405, "Method Not Allowed").await?;
        return Ok(());
    };

    let Some((host, port)) = split_authority(&authority) else {
        write_status(&mut stream, 400, "Bad Request").await?;
        return Ok(());
    };

    if !matches!(policy.matches_host(host), PolicyAction::Allow) {
        tracing::debug!(host, port, "proxy denied by policy");
        write_status(&mut stream, 403, "Forbidden").await?;
        return Ok(());
    }

    let mut target = match TcpStream::connect((host, port)).await {
        Ok(t) => t,
        Err(err) => {
            tracing::debug!(host, port, error = %err, "proxy upstream dial failed");
            write_status(&mut stream, 502, "Bad Gateway").await?;
            return Ok(());
        }
    };

    write_status(&mut stream, 200, "Connection Established").await?;

    let _ = tokio::io::copy_bidirectional(&mut stream, &mut target).await;
    Ok(())
}

async fn read_request_head(stream: &mut TcpStream) -> io::Result<String> {
    let (read, _write) = stream.split();
    let mut reader = BufReader::new(read);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "client closed before request head completed",
            ));
        }
        if head.len() + line.len() > MAX_REQUEST_HEAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request head exceeded maximum size",
            ));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
    }
    Ok(head)
}

fn parse_connect_authority(head: &str) -> Option<String> {
    let first_line = head.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?;
    let authority = parts.next()?;
    let _version = parts.next()?;
    if !method.eq_ignore_ascii_case("CONNECT") {
        return None;
    }
    Some(authority.to_string())
}

fn split_authority(authority: &str) -> Option<(&str, u16)> {
    let (host, port) = authority.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host, port))
}

async fn write_status(stream: &mut TcpStream, code: u16, reason: &str) -> io::Result<()> {
    let response = format!("HTTP/1.1 {code} {reason}\r\n\r\n");
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_connect_authority_accepts_valid_connect() {
        let head = "CONNECT example.com:443 HTTP/1.1\r\nHost: example.com\r\n";
        assert_eq!(
            parse_connect_authority(head).as_deref(),
            Some("example.com:443")
        );
    }

    #[test]
    fn parse_connect_authority_rejects_non_connect_methods() {
        for head in [
            "GET http://example.com/ HTTP/1.1\r\n",
            "POST /path HTTP/1.1\r\n",
            "\r\n",
            "",
        ] {
            assert!(parse_connect_authority(head).is_none(), "head: {head:?}");
        }
    }

    #[test]
    fn split_authority_parses_host_port() {
        assert_eq!(
            split_authority("example.com:443"),
            Some(("example.com", 443))
        );
        assert_eq!(
            split_authority("sub.example.com:8080"),
            Some(("sub.example.com", 8080))
        );
    }

    #[test]
    fn split_authority_rejects_missing_or_invalid_port() {
        assert!(split_authority("example.com").is_none());
        assert!(split_authority("example.com:").is_none());
        assert!(split_authority("example.com:abc").is_none());
        assert!(split_authority(":443").is_none());
    }

    #[tokio::test]
    async fn start_rejects_log_default_action() {
        let policy = NetworkPolicy {
            default_action: PolicyAction::Log,
            rules: vec![],
        };
        match start(policy).await {
            Err(err) => assert_eq!(err.kind(), io::ErrorKind::InvalidInput),
            Ok(_) => panic!("Log default must be rejected"),
        }
    }

    /// Relies on the cfg(test) override shortening `REQUEST_HEAD_TIMEOUT`
    /// to 250ms so the test doesn't wait 10s of wall-clock.
    #[tokio::test]
    async fn request_head_timeout_returns_408() {
        use tokio::io::AsyncReadExt;

        let handle = start(NetworkPolicy::allow_all()).await.unwrap();
        let mut client = tokio::net::TcpStream::connect(handle.listen_addr())
            .await
            .unwrap();

        // Send a partial request line and never finish it.
        client
            .write_all(b"CONNECT example.com:443 HT")
            .await
            .unwrap();
        client.flush().await.unwrap();

        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        // Give the proxy time past its head-read timeout to emit 408.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let n = client.read(&mut byte).await.unwrap();
            if n == 0 {
                break;
            }
            head.push(byte[0]);
            if head.ends_with(b"\r\n\r\n") {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "proxy did not emit response within deadline"
            );
        }
        let text = String::from_utf8(head).unwrap();
        assert!(
            text.starts_with("HTTP/1.1 408 "),
            "expected 408 Request Timeout, got: {text:?}"
        );
    }
}
