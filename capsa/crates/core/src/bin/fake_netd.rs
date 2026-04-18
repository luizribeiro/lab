//! Fake `capsa-netd` for orchestrate tests. Parses the launch spec,
//! (optionally) signals readiness, answers `AddInterface` requests
//! with `Ok`, and then either exits after a delay or hangs until
//! killed. Behaviour is driven by environment variables so the
//! orchestrate test harness can script specific scenarios.

use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::thread;
use std::time::Duration;

use capsa_control::{recv_request, send_response, IncomingRequest};
use capsa_spec::{parse_launch_spec_args, ControlResponse, NetLaunchSpec};

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
        match recv_request(fd.as_raw_fd()) {
            Ok(None) => return,
            Ok(Some(IncomingRequest::Parsed { .. } | IncomingRequest::Malformed(_))) => {
                if let Err(err) = send_response(fd.as_raw_fd(), &ControlResponse::Ok) {
                    eprintln!("fake-netd: send_response error: {err}");
                    return;
                }
            }
            Err(err) => {
                eprintln!("fake-netd: recv_request error: {err}");
                return;
            }
        }
    }
}
