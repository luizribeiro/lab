//! Shared test helpers for capsa integration tests.
//!
//! Crate-private to the workspace (`publish = false`); pulled in via
//! `dev-dependencies` from any test that needs an RAII child guard
//! or a stdio drain thread, so we don't grow a fourth copy of these
//! 30-line snippets across `crates/sandbox/tests`,
//! `crates/netd/tests`, and `crates/cli/tests`.

use std::io::Read;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;

/// Returns a [`capsa_sandbox::SandboxBuilder`] with `CAPSA_SYD_PATH`
/// and `CAPSA_LIBRARY_DIRS` applied. Use this in integration tests
/// that spawn dynamically-linked binaries inside the sandbox.
pub fn sandbox_builder() -> capsa_sandbox::SandboxBuilder {
    let mut builder = capsa_sandbox::Sandbox::builder();
    if let Some(val) = std::env::var_os("CAPSA_SYD_PATH") {
        builder = builder.syd_path(std::path::PathBuf::from(val));
    }
    if let Some(val) = std::env::var_os("CAPSA_LIBRARY_DIRS") {
        for dir in std::env::split_paths(&val) {
            if !dir.as_os_str().is_empty() {
                builder = builder.library_path(dir);
            }
        }
    }
    builder
}

/// RAII guard that SIGKILLs and reaps a spawned child on drop, so a
/// panicking `#[test]` body never leaks a subprocess. Two flavors:
///
/// * [`ChildGuard::new`] kills only the wrapped child.
/// * [`ChildGuard::with_pgroup`] kills the child's process group,
///   for cases where the child has spawned its own grandchildren
///   that inherit the same stdio pipes (capsa CLI → vmm + netd is
///   the canonical example: killing only the CLI leaves the
///   grandchildren holding the pipes open and any drain thread
///   reading from them blocks on `read()` forever).
pub struct ChildGuard {
    pub child: Child,
    pgid: Option<i32>,
}

impl ChildGuard {
    pub fn new(child: Child) -> Self {
        Self { child, pgid: None }
    }

    pub fn with_pgroup(child: Child, pgid: i32) -> Self {
        Self {
            child,
            pgid: Some(pgid),
        }
    }

    /// Send SIGKILL to the wrapped child (or its process group, if
    /// the guard was registered via [`ChildGuard::with_pgroup`]).
    /// Use this *before* joining drain threads on the child's
    /// stdio pipes; the drain threads only see EOF once every
    /// holder of those pipes is gone.
    pub fn kill_now(&mut self) {
        match self.pgid {
            Some(pgid) => kill_pgroup(pgid),
            None => {
                let _ = self.child.kill();
            }
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.kill_now();
        let _ = self.child.wait();
    }
}

fn kill_pgroup(pgid: i32) {
    // SAFETY: `pgid` is an integer; killpg is async-signal-safe and
    // reports any error via errno. ESRCH (group already empty) is
    // the only expected error and is harmless, so we discard the
    // result.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

/// Spawn a thread that reads `reader` until EOF and appends every
/// chunk to `sink`. The returned `JoinHandle` only exits once
/// `reader` returns EOF, so callers must close (or kill the writer
/// of) the underlying fd before joining.
pub fn spawn_drain<R>(mut reader: R, sink: Arc<Mutex<Vec<u8>>>) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut guard) = sink.lock() {
                        guard.extend_from_slice(&buf[..n]);
                    }
                }
            }
        }
    })
}
