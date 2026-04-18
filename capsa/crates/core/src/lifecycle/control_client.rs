//! Client side of the netd control protocol. Sends `AddInterface`
//! requests over a `SOCK_SEQPACKET` control socket, transferring the
//! host-side fd via `SCM_RIGHTS` ancillary data, and reads the
//! daemon's response.

#![allow(dead_code)]

use std::io::{IoSlice, IoSliceMut};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};

use anyhow::{bail, Context, Result};
use capsa_spec::{ControlRequest, ControlResponse};
use nix::cmsg_space;
use nix::sys::socket::{recvmsg, sendmsg, ControlMessage, MsgFlags};

const MAX_RESPONSE_LEN: usize = 64 * 1024;

pub(super) struct ControlClient {
    fd: OwnedFd,
}

impl ControlClient {
    pub(super) fn new(fd: OwnedFd) -> Self {
        Self { fd }
    }

    pub(super) fn send_add_interface(
        &mut self,
        mac: [u8; 6],
        port_forwards: Vec<(u16, u16)>,
        host_fd: &impl AsRawFd,
    ) -> Result<()> {
        let request = ControlRequest::AddInterface { mac, port_forwards };
        let body = serde_json::to_vec(&request).context("serialize AddInterface request")?;
        let fds = [host_fd.as_raw_fd()];
        let iov = [IoSlice::new(&body)];
        let cmsgs = [ControlMessage::ScmRights(&fds)];

        sendmsg::<()>(self.fd.as_raw_fd(), &iov, &cmsgs, MsgFlags::empty(), None)
            .map_err(|errno| std::io::Error::from_raw_os_error(errno as i32))
            .context("sendmsg AddInterface")?;

        let response = self.recv_response().context("read AddInterface response")?;
        match response {
            ControlResponse::Ok => Ok(()),
            ControlResponse::Error { message } => {
                bail!("netd rejected AddInterface: {message}")
            }
        }
    }

    fn recv_response(&mut self) -> Result<ControlResponse> {
        let mut buf = vec![0u8; MAX_RESPONSE_LEN];
        let mut cmsg = cmsg_space!([RawFd; 1]);
        let bytes = {
            let mut iov = [IoSliceMut::new(&mut buf)];
            let msg = recvmsg::<()>(
                self.fd.as_raw_fd(),
                &mut iov,
                Some(&mut cmsg),
                MsgFlags::empty(),
            )
            .map_err(|errno| std::io::Error::from_raw_os_error(errno as i32))
            .context("recvmsg response")?;
            if msg.bytes == 0 {
                bail!("netd control socket closed before sending response");
            }
            msg.bytes
        };
        serde_json::from_slice(&buf[..bytes]).context("deserialize ControlResponse")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::socket::{
        recvmsg, socketpair, AddressFamily, ControlMessageOwned, SockFlag, SockType,
    };
    use std::os::fd::FromRawFd;

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
        .expect("dummy pair");
        a
    }

    fn recv_request_and_fd(fd: RawFd) -> (ControlRequest, Option<OwnedFd>) {
        let mut buf = vec![0u8; 4096];
        let mut cmsg = cmsg_space!([RawFd; 1]);
        let (bytes, received_fd) = {
            let mut iov = [IoSliceMut::new(&mut buf)];
            let msg =
                recvmsg::<()>(fd, &mut iov, Some(&mut cmsg), MsgFlags::empty()).expect("recvmsg");
            let bytes = msg.bytes;
            let mut received_fd: Option<OwnedFd> = None;
            for cmsg in msg.cmsgs().expect("cmsgs") {
                if let ControlMessageOwned::ScmRights(fds) = cmsg {
                    if let Some(&raw) = fds.first() {
                        // SAFETY: kernel transferred this fd to us.
                        received_fd = Some(unsafe { OwnedFd::from_raw_fd(raw) });
                    }
                }
            }
            (bytes, received_fd)
        };
        let request: ControlRequest =
            serde_json::from_slice(&buf[..bytes]).expect("deserialize request");
        (request, received_fd)
    }

    fn send_response_sync(fd: RawFd, response: &ControlResponse) {
        let body = serde_json::to_vec(response).unwrap();
        let iov = [IoSlice::new(&body)];
        sendmsg::<()>(fd, &iov, &[], MsgFlags::empty(), None).expect("sendmsg response");
    }

    #[test]
    fn send_add_interface_round_trips_mac_forwards_and_fd() {
        let (client_fd, server_fd) = seqpacket_pair();
        let dummy = dummy_fd();

        let server_raw = server_fd.as_raw_fd();
        let server_handle = std::thread::spawn(move || {
            let (request, host_fd) = recv_request_and_fd(server_raw);
            send_response_sync(server_raw, &ControlResponse::Ok);
            (request, host_fd)
        });

        let mut client = ControlClient::new(client_fd);
        client
            .send_add_interface(
                [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                vec![(8080, 80), (8443, 443)],
                &dummy,
            )
            .expect("send_add_interface should succeed");

        let (request, host_fd) = server_handle.join().expect("server thread");
        drop(server_fd);

        match request {
            ControlRequest::AddInterface { mac, port_forwards } => {
                assert_eq!(mac, [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]);
                assert_eq!(port_forwards, vec![(8080, 80), (8443, 443)]);
            }
        }
        assert!(host_fd.is_some(), "server should have received the host fd");
    }

    #[test]
    fn error_response_surfaces_as_err() {
        let (client_fd, server_fd) = seqpacket_pair();
        let dummy = dummy_fd();

        let server_raw = server_fd.as_raw_fd();
        let server_handle = std::thread::spawn(move || {
            let (_request, _host_fd) = recv_request_and_fd(server_raw);
            send_response_sync(
                server_raw,
                &ControlResponse::Error {
                    message: "pool exhausted".into(),
                },
            );
        });

        let mut client = ControlClient::new(client_fd);
        let err = client
            .send_add_interface([0x02, 0, 0, 0, 0, 1], vec![], &dummy)
            .expect_err("error response should fail");
        assert!(err.to_string().contains("pool exhausted"));

        server_handle.join().expect("server thread");
        drop(server_fd);
    }

    #[test]
    fn peer_close_before_response_fails_with_clear_error() {
        let (client_fd, server_fd) = seqpacket_pair();
        let dummy = dummy_fd();

        let server_handle = std::thread::spawn(move || {
            let _ = recv_request_and_fd(server_fd.as_raw_fd());
            drop(server_fd);
        });

        let mut client = ControlClient::new(client_fd);
        let err = client
            .send_add_interface([0x02, 0, 0, 0, 0, 1], vec![], &dummy)
            .expect_err("peer close should fail");
        assert!(
            err.to_string().contains("closed") || err.to_string().contains("read"),
            "unexpected: {err}"
        );

        server_handle.join().expect("server thread");
    }
}
