//! End-to-end darwin launch coverage: drives the `capsa` CLI binary
//! through the real `start::macos` path so a regression in the
//! sandbox + fd-chain orchestration would fail this test.
//!
//! Ignored by default because the signed `capsa-vmm` (which carries
//! the `com.apple.security.hypervisor` entitlement) is only produced
//! by `nix build .#capsa`. CI / local runs opt in by setting
//! `CAPSA_VMM_PATH`, `CAPSA_NETD_PATH`, `CAPSA_KERNEL`, and
//! `CAPSA_INITRAMFS` and invoking `cargo test --ignored`.

#![cfg(target_os = "macos")]

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const CLI_BIN: &str = env!("CARGO_BIN_EXE_capsa");
const BOOT_TIMEOUT: Duration = Duration::from_secs(45);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const BOOT_MARKER: &str = "Run /init as init process";
const DHCP_MARKER: &str = "udhcpc: lease of";

/// RAII guard that kills the entire process group on drop. The
/// capsa CLI spawns capsa-vmm and capsa-netd as grandchildren that
/// inherit its stdio pipes; killing only the CLI leaves the pipes
/// open and deadlocks the drain threads. Killing the pgroup is the
/// only way to release them in one shot.
struct ChildGuard {
    child: Child,
    pgid: i32,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        // wait() reaps the CLI's own exit status and does not block
        // on the grandchildren or on any pipe state, so it returns
        // promptly once killpg has delivered SIGKILL.
        kill_pgroup(self.pgid);
        let _ = self.child.wait();
    }
}

fn kill_pgroup(pgid: i32) {
    // SAFETY: `pgid` was set via `process_group(0)` so it equals the
    // CLI's own pid and names a real process group. killpg is
    // async-signal-safe; ESRCH (group already empty) is the only
    // expected error and is harmless, so we discard the result.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

#[test]
#[ignore]
fn cli_boots_no_network_vm_under_sandbox() {
    drive_cli(&required_env(), &[], BOOT_MARKER);
}

#[test]
#[ignore]
fn cli_boots_networked_vm_with_dhcp_lease() {
    drive_cli(&required_env(), &["--allow-host", "*"], DHCP_MARKER);
}

struct RequiredEnv {
    vmm: PathBuf,
    netd: PathBuf,
    kernel: PathBuf,
    initramfs: PathBuf,
}

/// Reads the four env vars the harness needs. Panics with an
/// instructional message if any are missing: opting in via
/// `--ignored` without setting the envs is always operator error,
/// and a silent pass would mask CI misconfiguration.
fn required_env() -> RequiredEnv {
    RequiredEnv {
        vmm: required_path("CAPSA_VMM_PATH"),
        netd: required_path("CAPSA_NETD_PATH"),
        kernel: required_path("CAPSA_KERNEL"),
        initramfs: required_path("CAPSA_INITRAMFS"),
    }
}

fn required_path(key: &str) -> PathBuf {
    match std::env::var_os(key) {
        Some(raw) if !raw.is_empty() => PathBuf::from(raw),
        _ => panic!(
            "{key} must be set for darwin e2e tests. Run via /tmp/phase3_darwin_e2e.sh, \
             or set CAPSA_VMM_PATH / CAPSA_NETD_PATH / CAPSA_KERNEL / CAPSA_INITRAMFS \
             to the nix-built signed binaries and vm-assets."
        ),
    }
}

fn drive_cli(env: &RequiredEnv, extra_args: &[&str], success_marker: &str) {
    let mut cmd = Command::new(CLI_BIN);
    cmd.env("CAPSA_VMM_PATH", &env.vmm)
        .env("CAPSA_NETD_PATH", &env.netd)
        .arg("--kernel")
        .arg(&env.kernel)
        .arg("--initramfs")
        .arg(&env.initramfs)
        .arg("--kernel-cmdline")
        .arg("console=hvc0 rdinit=/init")
        .arg("--vcpus")
        .arg("1")
        .arg("--memory-mib")
        .arg("512")
        .arg("-v")
        .args(extra_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);

    let mut child = cmd.spawn().expect("spawn capsa cli");
    let pgid = child.id() as i32;
    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");
    let mut guard = ChildGuard { child, pgid };

    let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
    let out_handle = spawn_drain(stdout, Arc::clone(&captured));
    let err_handle = spawn_drain(stderr, Arc::clone(&captured));

    // The CLI never exits naturally once the guest is up: capsa just
    // sits in `wait_either` until something dies. So poll for the
    // success marker and kill the pgroup ourselves on a hit.
    let deadline = Instant::now() + BOOT_TIMEOUT;
    let observed = loop {
        if contains(&captured.lock().unwrap(), success_marker) {
            break true;
        }
        if let Ok(Some(_)) = guard.child.try_wait() {
            break contains(&captured.lock().unwrap(), success_marker);
        }
        if Instant::now() >= deadline {
            break false;
        }
        thread::sleep(POLL_INTERVAL);
    };

    // Kill the pgroup before joining drain threads: capsa-vmm and
    // capsa-netd are grandchildren that hold the same stdio pipes,
    // so killing only the CLI leaves the pipes open and the drain
    // threads block on read() forever.
    kill_pgroup(guard.pgid);
    let _ = out_handle.join();
    let _ = err_handle.join();

    let log = String::from_utf8_lossy(&captured.lock().unwrap()).into_owned();
    assert!(
        observed,
        "did not observe `{success_marker}` within {BOOT_TIMEOUT:?}\n--- captured output ---\n{log}"
    );
}

fn contains(buf: &[u8], needle: &str) -> bool {
    String::from_utf8_lossy(buf).contains(needle)
}

fn spawn_drain<R: Read + Send + 'static>(
    mut reader: R,
    sink: Arc<Mutex<Vec<u8>>>,
) -> thread::JoinHandle<()> {
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
