use std::io::Read;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;

/// Returns a [`lockin::SandboxBuilder`] with `LOCKIN_TEST_EXEC_DIRS`
/// applied as recursive exec directories — the shared test harness
/// needs runtime libraries (e.g. libiconv from `/nix/store`) reachable
/// so dynamically-linked probes can launch under the sandbox.
pub fn sandbox_builder() -> lockin::SandboxBuilder {
    let mut builder = lockin::Sandbox::builder();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for dir in std::env::split_paths(&val) {
            if !dir.as_os_str().is_empty() && dir.is_absolute() {
                builder = builder.exec_dir(dir);
            }
        }
    }
    builder
}

/// RAII guard that SIGKILLs and reaps a spawned child on drop, so a
/// panicking `#[test]` body never leaks a subprocess.
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
    // SAFETY: killpg is async-signal-safe; ESRCH is harmless.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

/// Spawn a thread that reads `reader` until EOF and appends every
/// chunk to `sink`.
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
