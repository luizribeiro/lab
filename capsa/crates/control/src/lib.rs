//! Shared sync helpers for the capsa-netd control protocol.
//!
//! The protocol is a framed `SOCK_SEQPACKET` exchange:
//! - Clients send a JSON-serialized [`ControlRequest`] per datagram,
//!   optionally attaching a single fd via `SCM_RIGHTS`.
//! - The daemon replies with a JSON-serialized [`ControlResponse`].
//!
//! This crate exposes blocking send/recv helpers for each side. Async
//! consumers wrap them in `AsyncFd::readable()`/`writable()` loops and
//! treat `WouldBlock` as retryable; see `capsa-netd::control` for the
//! reference wrapper.

use std::io::{self, IoSlice, IoSliceMut};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};

use capsa_spec::{ControlRequest, ControlResponse};
use nix::cmsg_space;
use nix::sys::socket::{recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags};

pub const MAX_MESSAGE_LEN: usize = 64 * 1024;

/// A received request packet, together with any fd that was passed
/// via `SCM_RIGHTS`. `Malformed` preserves the parse error text so
/// the caller can surface it in the error response.
pub enum IncomingRequest {
    Parsed {
        request: ControlRequest,
        fd: Option<OwnedFd>,
    },
    Malformed(String),
}

/// Send a `ControlRequest` with an optional SCM_RIGHTS fd attached.
/// Returns `WouldBlock` if the socket is non-blocking and the kernel
/// buffer is full.
pub fn send_request(
    fd: RawFd,
    request: &ControlRequest,
    fd_to_pass: Option<RawFd>,
) -> io::Result<()> {
    let body = serde_json::to_vec(request)
        .map_err(|e| io::Error::other(format!("serialize ControlRequest: {e}")))?;
    let fds: [RawFd; 1] = [fd_to_pass.unwrap_or(-1)];
    let cmsgs_slice = [ControlMessage::ScmRights(&fds)];
    let cmsgs: &[ControlMessage<'_>] = if fd_to_pass.is_some() {
        &cmsgs_slice
    } else {
        &[]
    };
    let iov = [IoSlice::new(&body)];
    match sendmsg::<()>(fd, &iov, cmsgs, MsgFlags::empty(), None) {
        Ok(_) => Ok(()),
        Err(nix::errno::Errno::EAGAIN) => Err(io::ErrorKind::WouldBlock.into()),
        Err(err) => Err(io::Error::from_raw_os_error(err as i32)),
    }
}

/// Send a `ControlResponse`. Returns `WouldBlock` on EAGAIN.
pub fn send_response(fd: RawFd, response: &ControlResponse) -> io::Result<()> {
    let body = serde_json::to_vec(response)
        .map_err(|e| io::Error::other(format!("serialize ControlResponse: {e}")))?;
    let iov = [IoSlice::new(&body)];
    match sendmsg::<()>(fd, &iov, &[], MsgFlags::empty(), None) {
        Ok(_) => Ok(()),
        Err(nix::errno::Errno::EAGAIN) => Err(io::ErrorKind::WouldBlock.into()),
        Err(err) => Err(io::Error::from_raw_os_error(err as i32)),
    }
}

/// Receive a single request datagram. `Ok(None)` means the peer
/// closed cleanly; `Err(WouldBlock)` means the caller should retry.
pub fn recv_request(fd: RawFd) -> io::Result<Option<IncomingRequest>> {
    let mut buf = vec![0u8; MAX_MESSAGE_LEN];
    let mut cmsg = cmsg_space!([RawFd; 1]);

    let (bytes, received_fd) = {
        let mut iov = [IoSliceMut::new(&mut buf)];
        let msg = match recvmsg::<()>(fd, &mut iov, Some(&mut cmsg), MsgFlags::empty()) {
            Ok(m) => m,
            Err(nix::errno::Errno::EAGAIN) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(err) => return Err(io::Error::from_raw_os_error(err as i32)),
        };

        let bytes = msg.bytes;
        let mut received_fd: Option<OwnedFd> = None;
        for cmsg in msg.cmsgs().map_err(io::Error::other)? {
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

    match serde_json::from_slice::<ControlRequest>(&buf[..bytes]) {
        Ok(request) => Ok(Some(IncomingRequest::Parsed {
            request,
            fd: received_fd,
        })),
        Err(err) => {
            drop(received_fd);
            Ok(Some(IncomingRequest::Malformed(err.to_string())))
        }
    }
}

/// Receive a `ControlResponse`. Returns an error if the socket is
/// closed or the payload can't be deserialized.
pub fn recv_response(fd: RawFd) -> io::Result<ControlResponse> {
    let mut buf = vec![0u8; MAX_MESSAGE_LEN];
    let mut cmsg = cmsg_space!([RawFd; 1]);
    let bytes = {
        let mut iov = [IoSliceMut::new(&mut buf)];
        let msg = match recvmsg::<()>(fd, &mut iov, Some(&mut cmsg), MsgFlags::empty()) {
            Ok(m) => m,
            Err(nix::errno::Errno::EAGAIN) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(err) => return Err(io::Error::from_raw_os_error(err as i32)),
        };
        msg.bytes
    };
    if bytes == 0 {
        return Err(io::Error::other(
            "netd control socket closed before sending response",
        ));
    }
    serde_json::from_slice(&buf[..bytes])
        .map_err(|e| io::Error::other(format!("deserialize ControlResponse: {e}")))
}
