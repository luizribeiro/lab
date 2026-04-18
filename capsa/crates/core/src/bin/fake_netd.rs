//! Fake `capsa-netd` for orchestrate tests. Parses the launch spec,
//! (optionally) signals readiness, answers `AddInterface` requests
//! with `Ok`, and then either exits after a delay or hangs until
//! killed. Behaviour is driven by environment variables so the
//! orchestrate test harness can script specific scenarios.

use std::io::{IoSlice, IoSliceMut, Write};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::thread;
use std::time::Duration;

use capsa_spec::{parse_launch_spec_args, ControlRequest, ControlResponse, NetLaunchSpec};
use nix::cmsg_space;
use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};

const READY_SIGNAL: u8 = b'R';

fn main() {
    let launch_spec: NetLaunchSpec = parse_launch_spec_args(std::env::args().skip(1))
        .expect("fake-netd: failed to parse launch spec");
    launch_spec
        .validate()
        .expect("fake-netd: launch spec validation failed");

    if let Ok(path) = std::env::var("FAKE_NETD_PID_FILE") {
        std::fs::write(&path, std::process::id().to_string())
            .unwrap_or_else(|e| panic!("fake-netd: failed writing pid file {path}: {e}"));
    }

    if std::env::var("FAKE_NETD_TRAP_SIGTERM").is_ok() {
        // SAFETY: signal() with SIG_IGN is safe.
        unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) };
    }

    if let Some(control_fd) = launch_spec.control_fd {
        thread::spawn(move || control_loop(control_fd));
    }

    if std::env::var("FAKE_NETD_SKIP_READY").is_err() {
        signal_ready(launch_spec.ready_fd);
    }

    if let Ok(ms) = std::env::var("FAKE_NETD_EXIT_AFTER_READY_MS") {
        let ms: u64 = ms
            .parse()
            .expect("FAKE_NETD_EXIT_AFTER_READY_MS must be u64");
        thread::sleep(Duration::from_millis(ms));
        std::process::exit(42);
    }

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn signal_ready(ready_fd: i32) {
    // SAFETY: the launcher inherits a valid writable fd for us.
    let mut f = unsafe { std::fs::File::from_raw_fd(ready_fd) };
    f.write_all(&[READY_SIGNAL])
        .expect("fake-netd: failed to signal readiness");
    f.flush().ok();
    // Keep the fd open for the lifetime of the process so the
    // parent's poll doesn't see a premature close.
    std::mem::forget(f);
}

fn control_loop(raw_fd: i32) {
    // SAFETY: `raw_fd` is inherited from the launcher.
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    loop {
        match recv_once(fd.as_fd_raw()) {
            Ok(None) => return,
            Ok(Some(_request)) => {
                send_ok(fd.as_fd_raw());
            }
            Err(err) => {
                eprintln!("fake-netd: control recv error: {err}");
                return;
            }
        }
    }
}

trait AsFdRaw {
    fn as_fd_raw(&self) -> RawFd;
}

impl AsFdRaw for OwnedFd {
    fn as_fd_raw(&self) -> RawFd {
        std::os::fd::AsRawFd::as_raw_fd(self)
    }
}

fn recv_once(fd: RawFd) -> std::io::Result<Option<ControlRequest>> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut cmsg = cmsg_space!([RawFd; 1]);
    let (bytes, received) = {
        let mut iov = [IoSliceMut::new(&mut buf)];
        let msg = recvmsg::<()>(fd, &mut iov, Some(&mut cmsg), MsgFlags::empty())
            .map_err(|errno| std::io::Error::from_raw_os_error(errno as i32))?;
        let bytes = msg.bytes;
        let mut received: Option<RawFd> = None;
        for cmsg in msg.cmsgs().map_err(std::io::Error::other)? {
            if let ControlMessageOwned::ScmRights(fds) = cmsg {
                if let Some(&raw) = fds.first() {
                    received = Some(raw);
                }
            }
        }
        (bytes, received)
    };

    if let Some(raw) = received {
        // SAFETY: kernel handed this fd to us; drop by closing.
        unsafe {
            libc::close(raw);
        }
    }

    if bytes == 0 {
        return Ok(None);
    }

    let request: ControlRequest = serde_json::from_slice(&buf[..bytes])
        .map_err(|e| std::io::Error::other(format!("parse request: {e}")))?;
    Ok(Some(request))
}

fn send_ok(fd: RawFd) {
    let body = serde_json::to_vec(&ControlResponse::Ok).expect("serialize Ok");
    let iov = [IoSlice::new(&body)];
    if let Err(err) = sendmsg::<()>(fd, &iov, &[], MsgFlags::empty(), None) {
        eprintln!("fake-netd: sendmsg response error: {err}");
    }
}
