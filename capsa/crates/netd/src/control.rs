//! Runtime control socket for dynamically attaching guest interfaces
//! to a running netd. Reads `ControlRequest` messages over a
//! `SOCK_SEQPACKET` control socket, extracts the accompanying
//! host-side fd via `SCM_RIGHTS`, and dispatches to a handler.

#![allow(dead_code)]

use std::future::Future;
use std::io::{IoSlice, IoSliceMut};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use anyhow::{Context, Result};
use capsa_spec::{ControlRequest, ControlResponse};
use nix::cmsg_space;
use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;

const MAX_REQUEST_LEN: usize = 64 * 1024;

/// An `AddInterface` request after the host-side fd has been
/// extracted from the `SCM_RIGHTS` ancillary data.
pub struct AttachInterface {
    pub mac: [u8; 6],
    pub port_forwards: Vec<(u16, u16)>,
    pub host_fd: OwnedFd,
}

/// Run the control loop until the peer closes the socket. `handler`
/// is invoked for every successful `AddInterface` request; its
/// return value is sent back as the [`ControlResponse`].
pub async fn run_control_loop<F, Fut>(control_fd: OwnedFd, mut handler: F) -> Result<()>
where
    F: FnMut(AttachInterface) -> Fut,
    Fut: Future<Output = ControlResponse>,
{
    set_nonblocking(control_fd.as_raw_fd()).context("set control fd nonblocking")?;

    let async_fd = AsyncFd::with_interest(control_fd, Interest::READABLE | Interest::WRITABLE)
        .context("wrap control fd in AsyncFd")?;

    loop {
        match recv_next(&async_fd).await? {
            None => return Ok(()),
            Some(Incoming::AddInterface { request, host_fd }) => {
                let ControlRequest::AddInterface { mac, port_forwards } = request;
                let response = match host_fd {
                    Some(fd) => {
                        handler(AttachInterface {
                            mac,
                            port_forwards,
                            host_fd: fd,
                        })
                        .await
                    }
                    None => ControlResponse::Error {
                        message: "AddInterface requires an SCM_RIGHTS fd".into(),
                    },
                };
                send_response(&async_fd, &response).await?;
            }
            Some(Incoming::Malformed(err)) => {
                send_response(
                    &async_fd,
                    &ControlResponse::Error {
                        message: format!("malformed request: {err}"),
                    },
                )
                .await?;
            }
        }
    }
}

enum Incoming {
    AddInterface {
        request: ControlRequest,
        host_fd: Option<OwnedFd>,
    },
    Malformed(String),
}

async fn recv_next(async_fd: &AsyncFd<OwnedFd>) -> Result<Option<Incoming>> {
    loop {
        let mut guard = async_fd
            .readable()
            .await
            .context("wait readable on control fd")?;
        match guard.try_io(|inner| recv_sync(inner.get_ref().as_raw_fd())) {
            Ok(Ok(Some(incoming))) => return Ok(Some(incoming)),
            Ok(Ok(None)) => return Ok(None),
            Ok(Err(err)) => return Err(err.into()),
            Err(_would_block) => continue,
        }
    }
}

async fn send_response(async_fd: &AsyncFd<OwnedFd>, response: &ControlResponse) -> Result<()> {
    let body = serde_json::to_vec(response).context("serialize response")?;
    loop {
        let mut guard = async_fd
            .writable()
            .await
            .context("wait writable on control fd")?;
        match guard.try_io(|inner| send_sync(inner.get_ref().as_raw_fd(), &body)) {
            Ok(res) => return res.map_err(Into::into),
            Err(_would_block) => continue,
        }
    }
}

fn recv_sync(fd: RawFd) -> std::io::Result<Option<Incoming>> {
    let mut buf = vec![0u8; MAX_REQUEST_LEN];
    let mut cmsg = cmsg_space!([RawFd; 1]);

    let (bytes, received_fd) = {
        let mut iov = [IoSliceMut::new(&mut buf)];
        let msg = match recvmsg::<()>(fd, &mut iov, Some(&mut cmsg), MsgFlags::empty()) {
            Ok(m) => m,
            Err(nix::errno::Errno::EAGAIN) => {
                return Err(std::io::ErrorKind::WouldBlock.into());
            }
            Err(err) => return Err(std::io::Error::from_raw_os_error(err as i32)),
        };

        let bytes = msg.bytes;
        let mut received_fd: Option<OwnedFd> = None;
        for cmsg in msg.cmsgs().map_err(std::io::Error::other)? {
            if let ControlMessageOwned::ScmRights(fds) = cmsg {
                for &raw in &fds {
                    if received_fd.is_some() {
                        // SAFETY: kernel handed this fd to us; close extras.
                        unsafe {
                            libc::close(raw);
                        }
                        continue;
                    }
                    // SAFETY: fd was just transferred to us by the kernel.
                    received_fd = Some(unsafe { OwnedFd::from_raw_fd(raw) });
                }
            }
        }
        (bytes, received_fd)
    };

    if bytes == 0 {
        return Ok(None);
    }

    let payload = &buf[..bytes];
    match serde_json::from_slice::<ControlRequest>(payload) {
        Ok(request) => Ok(Some(Incoming::AddInterface {
            request,
            host_fd: received_fd,
        })),
        Err(err) => {
            drop(received_fd);
            Ok(Some(Incoming::Malformed(err.to_string())))
        }
    }
}

fn send_sync(fd: RawFd, body: &[u8]) -> std::io::Result<()> {
    let iov = [IoSlice::new(body)];
    match sendmsg::<()>(fd, &iov, &[], MsgFlags::empty(), None) {
        Ok(_) => Ok(()),
        Err(nix::errno::Errno::EAGAIN) => Err(std::io::ErrorKind::WouldBlock.into()),
        Err(err) => Err(std::io::Error::from_raw_os_error(err as i32)),
    }
}

fn set_nonblocking(fd: RawFd) -> std::io::Result<()> {
    // SAFETY: F_GETFL/F_SETFL on a valid fd is well-defined.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let rc = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::time::Duration;

    use nix::sys::socket::{socketpair, AddressFamily, ControlMessage, SockFlag, SockType};
    use tokio::sync::Mutex;

    fn seqpacket_pair() -> (OwnedFd, OwnedFd) {
        socketpair(
            AddressFamily::Unix,
            SockType::SeqPacket,
            None,
            SockFlag::SOCK_CLOEXEC,
        )
        .expect("seqpacket pair")
    }

    fn dummy_fd() -> OwnedFd {
        let (a, _b) = socketpair(
            AddressFamily::Unix,
            SockType::Datagram,
            None,
            SockFlag::SOCK_CLOEXEC,
        )
        .expect("datagram pair for dummy fd");
        a
    }

    fn send_request_sync(fd: RawFd, request: &ControlRequest, fds: &[RawFd]) {
        let body = serde_json::to_vec(request).expect("serialize");
        let iov = [IoSlice::new(&body)];
        let cmsgs = if fds.is_empty() {
            vec![]
        } else {
            vec![ControlMessage::ScmRights(fds)]
        };
        sendmsg::<()>(fd, &iov, &cmsgs, MsgFlags::empty(), None).expect("sendmsg");
    }

    fn send_raw_sync(fd: RawFd, body: &[u8]) {
        let iov = [IoSlice::new(body)];
        sendmsg::<()>(fd, &iov, &[], MsgFlags::empty(), None).expect("sendmsg");
    }

    fn recv_response_sync(fd: RawFd) -> ControlResponse {
        let mut buf = vec![0u8; 4096];
        let mut cmsg = cmsg_space!([RawFd; 1]);
        let bytes = {
            let mut iov = [IoSliceMut::new(&mut buf)];
            let msg =
                recvmsg::<()>(fd, &mut iov, Some(&mut cmsg), MsgFlags::empty()).expect("recvmsg");
            msg.bytes
        };
        serde_json::from_slice(&buf[..bytes]).expect("response should deserialize")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatches_add_interface_with_host_fd() {
        let (server, client) = seqpacket_pair();
        let dummy = dummy_fd();

        let received: Arc<Mutex<Vec<AttachInterface>>> = Arc::new(Mutex::new(Vec::new()));
        let received_tx = received.clone();

        let loop_task = tokio::spawn(async move {
            run_control_loop(server, move |iface| {
                let received = received_tx.clone();
                async move {
                    received.lock().await.push(iface);
                    ControlResponse::Ok
                }
            })
            .await
        });

        send_request_sync(
            client.as_raw_fd(),
            &ControlRequest::AddInterface {
                mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                port_forwards: vec![(8080, 80)],
            },
            &[dummy.as_raw_fd()],
        );

        let resp = recv_response_sync(client.as_raw_fd());
        assert_eq!(resp, ControlResponse::Ok);

        drop(client);
        let _ = tokio::time::timeout(Duration::from_secs(2), loop_task)
            .await
            .expect("loop should exit on peer close")
            .expect("loop task should not panic");

        let received = received.lock().await;
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].mac, [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]);
        assert_eq!(received[0].port_forwards, vec![(8080, 80)]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_host_fd_returns_error_response() {
        let (server, client) = seqpacket_pair();

        let loop_task = tokio::spawn(async move {
            run_control_loop(server, |_iface: AttachInterface| async {
                ControlResponse::Ok
            })
            .await
        });

        send_request_sync(
            client.as_raw_fd(),
            &ControlRequest::AddInterface {
                mac: [0x02, 0, 0, 0, 0, 1],
                port_forwards: vec![],
            },
            &[],
        );

        let resp = recv_response_sync(client.as_raw_fd());
        match resp {
            ControlResponse::Error { message } => {
                assert!(message.contains("SCM_RIGHTS"), "unexpected: {message}");
            }
            other => panic!("expected error response, got {other:?}"),
        }

        drop(client);
        let _ = tokio::time::timeout(Duration::from_secs(2), loop_task).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn malformed_payload_returns_error_response() {
        let (server, client) = seqpacket_pair();

        let loop_task = tokio::spawn(async move {
            run_control_loop(server, |_iface: AttachInterface| async {
                ControlResponse::Ok
            })
            .await
        });

        send_raw_sync(client.as_raw_fd(), b"{not json");

        let resp = recv_response_sync(client.as_raw_fd());
        match resp {
            ControlResponse::Error { message } => {
                assert!(message.contains("malformed"), "unexpected: {message}");
            }
            other => panic!("expected error response, got {other:?}"),
        }

        drop(client);
        let _ = tokio::time::timeout(Duration::from_secs(2), loop_task).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn peer_close_exits_loop_cleanly() {
        let (server, client) = seqpacket_pair();

        let loop_task = tokio::spawn(async move {
            run_control_loop(server, |_iface: AttachInterface| async {
                ControlResponse::Ok
            })
            .await
        });

        drop(client);

        let result = tokio::time::timeout(Duration::from_secs(2), loop_task)
            .await
            .expect("loop should exit on peer close")
            .expect("loop task should not panic");
        result.expect("loop should return Ok");
    }
}
