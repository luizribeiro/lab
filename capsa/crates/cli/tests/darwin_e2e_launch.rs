//! End-to-end darwin launch coverage: drives the `capsa` CLI binary
//! through the real `start::macos` path so a regression in the
//! sandbox + fd-chain orchestration would fail this test.
//!
//! Ignored by default because libkrun on macOS requires the signed
//! `capsa-vmm` (which carries `com.apple.security.hypervisor`) plus
//! a kernel + initramfs. Both come from the nix build:
//!
//!     nix build .#capsa
//!     nix build -o /tmp/capsa-vm-assets .#vm-assets
//!
//! With those in place a bare `cargo test -p capsa-cli --ignored
//! -- --test-threads=1` runs the test against the workspace's
//! `result/libexec/capsa/` and `/tmp/capsa-vm-assets/`. CI can
//! override any path via `CAPSA_VMM_PATH`, `CAPSA_NETD_PATH`,
//! `CAPSA_KERNEL`, and `CAPSA_INITRAMFS`.
//!
//! `--test-threads=1` is required: HVF does not multiplex two
//! independent guests on a single host efficiently and the second
//! to start times out.

#![cfg(target_os = "macos")]

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use capsa_testkit::{spawn_drain, ChildGuard};

const CLI_BIN: &str = env!("CARGO_BIN_EXE_capsa");
const BOOT_TIMEOUT: Duration = Duration::from_secs(45);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const BOOT_MARKER: &str = "Run /init as init process";
const DHCP_MARKER: &str = "udhcpc: lease of";

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

/// Resolves the four required artifact paths. Each one is an env-var
/// override if set, otherwise a default location populated by the
/// nix build. Panics loudly with build instructions if a path is
/// neither set nor present, so a misconfigured CI run fails fast
/// instead of silently passing.
fn required_env() -> RequiredEnv {
    let nix_capsa_dir = workspace_root().join("result/libexec/capsa");
    let nix_assets_dir = PathBuf::from("/tmp/capsa-vm-assets");

    RequiredEnv {
        vmm: resolve_path(
            "CAPSA_VMM_PATH",
            nix_capsa_dir.join("capsa-vmm"),
            "nix build .#capsa",
        ),
        netd: resolve_path(
            "CAPSA_NETD_PATH",
            nix_capsa_dir.join("capsa-netd"),
            "nix build .#capsa",
        ),
        kernel: resolve_path(
            "CAPSA_KERNEL",
            nix_assets_dir.join("vmlinuz"),
            "nix build -o /tmp/capsa-vm-assets .#vm-assets",
        ),
        initramfs: resolve_path(
            "CAPSA_INITRAMFS",
            nix_assets_dir.join("initramfs.cpio.lz4"),
            "nix build -o /tmp/capsa-vm-assets .#vm-assets",
        ),
    }
}

fn resolve_path(env_key: &str, default: PathBuf, build_hint: &str) -> PathBuf {
    let candidate = match std::env::var_os(env_key) {
        Some(raw) if !raw.is_empty() => PathBuf::from(raw),
        _ => default,
    };
    if !candidate.exists() {
        panic!(
            "darwin e2e test artifact missing at {}. Either set {env_key} \
             or run `{build_hint}` from the workspace root.",
            candidate.display()
        );
    }
    candidate
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points at crates/cli; the workspace root is two up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR has at least two ancestors")
        .to_path_buf()
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
    let mut guard = ChildGuard::with_pgroup(child, pgid);

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
    guard.kill_now();
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
