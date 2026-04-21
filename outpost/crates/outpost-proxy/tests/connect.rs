//! End-to-end CONNECT proxy tests: spin up a real echo server, run
//! the proxy against it, and exercise the full byte-shuffling path.

use std::time::{Duration, Instant};

use outpost::NetworkPolicy;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Minimal echo server: accept connections, echo bytes back until the
/// peer closes. Returns its bound loopback port.
async fn start_echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let (mut r, mut w) = sock.split();
                let _ = tokio::io::copy(&mut r, &mut w).await;
            });
        }
    });
    port
}

/// Read bytes from `stream` until `\r\n\r\n`, parse the HTTP status
/// line, return `(code, reason)`.
async fn read_status(stream: &mut TcpStream) -> (u16, String) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await.unwrap();
        assert!(n > 0, "proxy closed before sending response head");
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        assert!(head.len() < 4096, "response head too long");
    }
    let text = String::from_utf8(head).unwrap();
    let line = text.lines().next().unwrap();
    let mut parts = line.split_whitespace();
    let _version = parts.next().unwrap();
    let code: u16 = parts.next().unwrap().parse().unwrap();
    let reason = parts.collect::<Vec<_>>().join(" ");
    (code, reason)
}

async fn send_connect(stream: &mut TcpStream, target: &str) -> (u16, String) {
    let request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    read_status(stream).await
}

#[tokio::test]
async fn connect_allow_tunnels_bytes_bidirectionally() {
    let echo_port = start_echo_server().await;
    let policy = NetworkPolicy::from_allowed_hosts(["localhost"]).unwrap();
    let handle = outpost_proxy::start(policy).await.unwrap();

    let mut client = TcpStream::connect(handle.listen_addr()).await.unwrap();
    let target = format!("localhost:{echo_port}");
    let (code, _) = send_connect(&mut client, &target).await;
    assert_eq!(code, 200, "CONNECT should succeed for allowlisted host");

    client.write_all(b"hello outpost").await.unwrap();
    client.flush().await.unwrap();
    let mut buf = [0u8; 13];
    tokio::time::timeout(Duration::from_secs(2), client.read_exact(&mut buf))
        .await
        .expect("echo timed out")
        .expect("echo read failed");
    assert_eq!(&buf, b"hello outpost");
}

#[tokio::test]
async fn connect_deny_returns_403_without_dialing_upstream() {
    let policy = NetworkPolicy::from_allowed_hosts(["allowed.example.com"]).unwrap();
    let handle = outpost_proxy::start(policy).await.unwrap();

    let mut client = TcpStream::connect(handle.listen_addr()).await.unwrap();
    let (code, _) = send_connect(&mut client, "evil.example.com:443").await;
    assert_eq!(code, 403);
}

#[tokio::test]
async fn non_connect_method_returns_405() {
    let handle = outpost_proxy::start(NetworkPolicy::allow_all())
        .await
        .unwrap();

    let mut client = TcpStream::connect(handle.listen_addr()).await.unwrap();
    client
        .write_all(b"GET http://example.com/ HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await
        .unwrap();
    let (code, _) = read_status(&mut client).await;
    assert_eq!(code, 405);
}

#[tokio::test]
async fn malformed_authority_returns_400() {
    let handle = outpost_proxy::start(NetworkPolicy::allow_all())
        .await
        .unwrap();

    let mut client = TcpStream::connect(handle.listen_addr()).await.unwrap();
    client
        .write_all(b"CONNECT noport HTTP/1.1\r\n\r\n")
        .await
        .unwrap();
    let (code, _) = read_status(&mut client).await;
    assert_eq!(code, 400);
}

#[tokio::test]
async fn connect_allow_returns_502_when_upstream_refuses() {
    // Policy allows 127.0.0.1 but port 1 is reserved and nothing
    // listens there, so the kernel should RST the outbound connect.
    // The proxy must translate that to 502 Bad Gateway, not 403.
    let policy = NetworkPolicy::from_allowed_hosts(["127.0.0.1"]).unwrap();
    let handle = outpost_proxy::start(policy).await.unwrap();

    let mut client = TcpStream::connect(handle.listen_addr()).await.unwrap();
    let (code, _) = send_connect(&mut client, "127.0.0.1:1").await;
    assert_eq!(code, 502);
}

#[tokio::test]
async fn dropping_handle_stops_accepting_connections() {
    let handle = outpost_proxy::start(NetworkPolicy::allow_all())
        .await
        .unwrap();
    let addr = handle.listen_addr();
    drop(handle);

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let result =
            tokio::time::timeout(Duration::from_millis(200), TcpStream::connect(addr)).await;
        match result {
            Err(_) | Ok(Err(_)) => return,
            Ok(Ok(_stream)) => {
                if Instant::now() >= deadline {
                    panic!("proxy still accepting connections after handle drop");
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}
