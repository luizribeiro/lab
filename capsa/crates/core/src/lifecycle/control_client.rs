//! Client side of the netd control protocol. Thin wrapper around
//! `capsa-control` that pairs an `AddInterface` send with reading
//! the daemon's response.

use std::os::fd::{AsRawFd, OwnedFd};

use anyhow::{bail, Context, Result};
use capsa_control::{recv_response, send_request};
use capsa_spec::{ControlRequest, ControlResponse};

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
        udp_forwards: Vec<(u16, u16)>,
        host_fd: &impl AsRawFd,
    ) -> Result<()> {
        let request = ControlRequest::AddInterface {
            mac,
            port_forwards,
            udp_forwards,
        };
        send_request(self.fd.as_raw_fd(), &request, Some(host_fd.as_raw_fd()))
            .context("sendmsg AddInterface")?;

        match recv_response(self.fd.as_raw_fd()).context("read AddInterface response")? {
            ControlResponse::Ok => Ok(()),
            ControlResponse::Error { message } => {
                bail!("netd rejected AddInterface: {message}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capsa_control::{recv_request, send_response, IncomingRequest};
    use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};

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

    #[test]
    fn send_add_interface_round_trips_mac_forwards_and_fd() {
        let (client_fd, server_fd) = seqpacket_pair();
        let dummy = dummy_fd();

        let server_raw = server_fd.as_raw_fd();
        let server_handle = std::thread::spawn(move || {
            let incoming = recv_request(server_raw)
                .expect("recv_request")
                .expect("peer closed");
            send_response(server_raw, &ControlResponse::Ok).expect("send_response");
            match incoming {
                IncomingRequest::Parsed { request, fd } => (request, fd),
                IncomingRequest::Malformed(err) => panic!("unexpected malformed: {err}"),
            }
        });

        let mut client = ControlClient::new(client_fd);
        client
            .send_add_interface(
                [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                vec![(8080, 80), (8443, 443)],
                vec![(5353, 53)],
                &dummy,
            )
            .expect("send_add_interface should succeed");

        let (request, host_fd) = server_handle.join().expect("server thread");
        drop(server_fd);

        match request {
            ControlRequest::AddInterface {
                mac,
                port_forwards,
                udp_forwards,
            } => {
                assert_eq!(mac, [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]);
                assert_eq!(port_forwards, vec![(8080, 80), (8443, 443)]);
                assert_eq!(udp_forwards, vec![(5353, 53)]);
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
            let _ = recv_request(server_raw).expect("recv_request");
            send_response(
                server_raw,
                &ControlResponse::Error {
                    message: "pool exhausted".into(),
                },
            )
            .expect("send_response");
        });

        let mut client = ControlClient::new(client_fd);
        let err = client
            .send_add_interface([0x02, 0, 0, 0, 0, 1], vec![], vec![], &dummy)
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
            let _ = recv_request(server_fd.as_raw_fd()).expect("recv_request");
            drop(server_fd);
        });

        let mut client = ControlClient::new(client_fd);
        let err = client
            .send_add_interface([0x02, 0, 0, 0, 0, 1], vec![], vec![], &dummy)
            .expect_err("peer close should fail");
        assert!(
            err.to_string().contains("closed") || err.to_string().contains("read"),
            "unexpected: {err}"
        );

        server_handle.join().expect("server thread");
    }
}
