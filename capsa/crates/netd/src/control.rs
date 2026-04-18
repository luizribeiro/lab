//! Async wrapper around the netd control socket. Delegates the
//! wire-format send/recv to `capsa-control`; adds the tokio
//! `AsyncFd` try_io loop and a dispatch handler.

use std::future::Future;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};

use anyhow::{Context, Result};
use capsa_control::{recv_request, send_response, IncomingRequest};
use capsa_spec::{ControlRequest, ControlResponse};
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;

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
            Some(IncomingRequest::Parsed {
                request: ControlRequest::AddInterface { mac, port_forwards },
                fd,
            }) => {
                let response = match fd {
                    Some(host_fd) => {
                        handler(AttachInterface {
                            mac,
                            port_forwards,
                            host_fd,
                        })
                        .await
                    }
                    None => ControlResponse::Error {
                        message: "AddInterface requires an SCM_RIGHTS fd".into(),
                    },
                };
                send_response_async(&async_fd, &response).await?;
            }
            Some(IncomingRequest::Malformed(err)) => {
                send_response_async(
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

async fn recv_next(async_fd: &AsyncFd<OwnedFd>) -> Result<Option<IncomingRequest>> {
    loop {
        let mut guard = async_fd
            .readable()
            .await
            .context("wait readable on control fd")?;
        match guard.try_io(|inner| recv_request(inner.get_ref().as_raw_fd())) {
            Ok(result) => return result.map_err(Into::into),
            Err(_would_block) => continue,
        }
    }
}

async fn send_response_async(
    async_fd: &AsyncFd<OwnedFd>,
    response: &ControlResponse,
) -> Result<()> {
    loop {
        let mut guard = async_fd
            .writable()
            .await
            .context("wait writable on control fd")?;
        match guard.try_io(|inner| send_response(inner.get_ref().as_raw_fd(), response)) {
            Ok(res) => return res.map_err(Into::into),
            Err(_would_block) => continue,
        }
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

    use capsa_control::{recv_response, send_request};
    use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
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

    fn send_raw_sync(fd: RawFd, body: &[u8]) {
        use std::io::IoSlice;
        let iov = [IoSlice::new(body)];
        nix::sys::socket::sendmsg::<()>(fd, &iov, &[], nix::sys::socket::MsgFlags::empty(), None)
            .expect("sendmsg");
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

        send_request(
            client.as_raw_fd(),
            &ControlRequest::AddInterface {
                mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                port_forwards: vec![(8080, 80)],
            },
            Some(dummy.as_raw_fd()),
        )
        .expect("send_request");

        let resp = recv_response(client.as_raw_fd()).expect("recv_response");
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

        send_request(
            client.as_raw_fd(),
            &ControlRequest::AddInterface {
                mac: [0x02, 0, 0, 0, 0, 1],
                port_forwards: vec![],
            },
            None,
        )
        .expect("send_request");

        let resp = recv_response(client.as_raw_fd()).expect("recv_response");
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

        let resp = recv_response(client.as_raw_fd()).expect("recv_response");
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
