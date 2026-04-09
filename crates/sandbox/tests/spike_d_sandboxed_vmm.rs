//! Spike D: boot libkrun+HVF through the darwin SandboxBuilder.
//!
//! Ignored by default; driven by `/tmp/spike_d_sandboxed_vmm.sh`, which
//! builds the nix artifacts and points the env vars below at them.

#![cfg(target_os = "macos")]

use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use capsa_sandbox::Sandbox;

mod common;
use common::ChildGuard;

const BOOT_TIMEOUT: Duration = Duration::from_secs(45);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[test]
#[ignore]
fn sandboxed_vmm_boots_under_hvf() {
    let vmm = env_path("CAPSA_VMM_BIN");
    let kernel = env_path("CAPSA_KERNEL");
    let initramfs = env_path("CAPSA_INITRAMFS");

    let spec = build_launch_spec(&kernel, &initramfs);

    // The `_sandbox_guard` owns the private_tmp directory; it must
    // outlive the spawned child.
    let (mut cmd, _sandbox_guard) = Sandbox::builder()
        .allow_network(false)
        .allow_kvm(true)
        .allow_interactive_tty(true)
        .read_only_path(vmm.clone())
        .read_only_path(kernel)
        .read_only_path(initramfs)
        .build(&vmm)
        .expect("sandbox builder build");

    cmd.arg("--launch-spec-json")
        .arg(&spec)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn sandboxed capsa-vmm");
    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");
    let mut guard = ChildGuard(child);

    let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
    let out_handle = spawn_drain(stdout, Arc::clone(&captured));
    let err_handle = spawn_drain(stderr, Arc::clone(&captured));

    // The guest vCPU keeps running indefinitely once /init starts, so
    // we can't wait for a natural exit — poll for a boot marker and
    // kill the child ourselves on success.
    let deadline = Instant::now() + BOOT_TIMEOUT;
    let matched = loop {
        if contains_boot_marker(&captured.lock().unwrap()) {
            break true;
        }
        if let Ok(Some(_)) = guard.0.try_wait() {
            break contains_boot_marker(&captured.lock().unwrap());
        }
        if Instant::now() >= deadline {
            break false;
        }
        thread::sleep(POLL_INTERVAL);
    };

    // Kill before joining the drain threads: they only see EOF once
    // the child closes its pipes.
    let _ = guard.0.kill();
    let _ = out_handle.join();
    let _ = err_handle.join();

    let captured_text = String::from_utf8_lossy(&captured.lock().unwrap()).into_owned();
    assert!(
        matched,
        "did not observe guest boot markers within {BOOT_TIMEOUT:?}\n\
         --- captured output ---\n{captured_text}"
    );
}

fn contains_boot_marker(buf: &[u8]) -> bool {
    let text = String::from_utf8_lossy(buf);
    text.contains("Run /init as init process") || text.contains("Freeing unused kernel memory")
}

fn env_path(key: &str) -> PathBuf {
    let value =
        std::env::var(key).unwrap_or_else(|_| panic!("{key} env var required for spike D test"));
    PathBuf::from(value)
}

fn build_launch_spec(kernel: &std::path::Path, initramfs: &std::path::Path) -> String {
    serde_json::json!({
        "root": null,
        "kernel": kernel,
        "initramfs": initramfs,
        "kernel_cmdline": "console=hvc0 rdinit=/init",
        "vcpus": 1,
        "memory_mib": 512,
        "verbosity": 1,
        "resolved_interfaces": [],
    })
    .to_string()
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
