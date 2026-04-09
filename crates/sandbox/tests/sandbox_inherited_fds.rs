//! Spike A regression coverage: inherited fds (beyond stdio) must
//! survive the platform sandbox wrapper exec.
//!
//! On macOS `Sandbox::builder().build(...)` uses `sandbox-exec` as an
//! outer process that execs into the target program. The
//! `configure_inherited_fds` pre_exec hook clears `FD_CLOEXEC` on the
//! parent-owned fds before the first exec, so they are live when
//! `sandbox-exec` starts; what we verify here is that `sandbox-exec`
//! itself does not close them before exec'ing the target program.
//!
//! On Linux the wrapper is `syd`, which we also cover here so the
//! regression test runs on both platforms.

mod common;

use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::Stdio;

use capsa_sandbox::Sandbox;

use common::probe_binary;

fn pipe_with_byte(byte: u8) -> OwnedFd {
    let (read_end, mut write_end) = std::io::pipe().expect("create pipe");
    write_end.write_all(&[byte]).expect("write marker byte");
    drop(write_end);
    read_end.into()
}

fn run_probe_with_pairs(builder_fds: Vec<(u8, OwnedFd)>) {
    let mut builder = Sandbox::builder();
    let mut pair_args: Vec<String> = Vec::with_capacity(builder_fds.len() * 2);
    for (expected_byte, owned) in builder_fds {
        let pre_raw = owned.as_raw_fd();
        let raw = builder
            .inherit_fd(owned)
            .unwrap_or_else(|e| panic!("inherit_fd({pre_raw}): {e}"));
        assert_eq!(
            raw, pre_raw,
            "inherit_fd returned a different raw fd than the one passed in"
        );
        pair_args.push(raw.to_string());
        pair_args.push(std::str::from_utf8(&[expected_byte]).unwrap().to_string());
    }

    let probe = probe_binary();
    let (mut cmd, _sandbox) = builder
        .build(&probe)
        .expect("build sandbox command for probe");
    cmd.arg("fd-read-byte")
        .args(&pair_args)
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    let status = cmd
        .status()
        .expect("spawn sandboxed probe for inherited fds");
    assert!(
        status.success(),
        "probe failed for inherited fds {pair_args:?}; status = {status:?}"
    );
}

#[test]
fn single_inherited_fd_survives_sandbox_wrapper() {
    run_probe_with_pairs(vec![(b'K', pipe_with_byte(b'K'))]);
}

#[test]
fn multiple_inherited_fds_survive_sandbox_wrapper() {
    run_probe_with_pairs(vec![
        (b'A', pipe_with_byte(b'A')),
        (b'B', pipe_with_byte(b'B')),
        (b'C', pipe_with_byte(b'C')),
    ]);
}
