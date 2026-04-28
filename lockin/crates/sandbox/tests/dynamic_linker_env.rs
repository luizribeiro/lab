//! Dynamic-linker env-var blocklist tests.
//!
//! The library guarantees that LD_PRELOAD / LD_LIBRARY_PATH /
//! LD_AUDIT / DYLD_INSERT_LIBRARIES / DYLD_LIBRARY_PATH /
//! DYLD_FRAMEWORK_PATH never reach the sandboxed child, no matter
//! how a Rust caller tried to set them: explicit `env()`, batched
//! `envs()`, or inheritance from the parent process's environment.

mod common;

use common::{probe_binary, sandbox_builder};

fn assert_unset_in_child(var: &str, set_on_cmd: bool, set_in_parent: bool) {
    let probe = probe_binary();

    let prior = std::env::var_os(var);
    if set_in_parent {
        std::env::set_var(var, "/tmp/lockin-test-evil.so");
    }

    let mut cmd = sandbox_builder()
        .command(&probe)
        .expect("build sandbox command");
    if set_on_cmd {
        cmd.env(var, "/tmp/lockin-test-evil.so");
    }
    let status = cmd
        .args(["env-var-unset", var])
        .status()
        .expect("run probe");

    if set_in_parent {
        match prior {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }

    assert!(
        status.success(),
        "{var} must not reach the sandboxed child (set_on_cmd={set_on_cmd}, set_in_parent={set_in_parent})"
    );
}

#[test]
fn ld_preload_set_via_rust_api_does_not_reach_child() {
    assert_unset_in_child("LD_PRELOAD", true, false);
}

#[test]
fn dyld_insert_libraries_set_via_rust_api_does_not_reach_child() {
    assert_unset_in_child("DYLD_INSERT_LIBRARIES", true, false);
}

#[test]
fn ld_library_path_inherited_from_parent_does_not_reach_child() {
    assert_unset_in_child("LD_LIBRARY_PATH", false, true);
}

#[test]
fn dyld_library_path_via_envs_batch_does_not_reach_child() {
    let probe = probe_binary();
    let mut cmd = sandbox_builder()
        .command(&probe)
        .expect("build sandbox command");
    cmd.envs([
        ("DYLD_LIBRARY_PATH", "/tmp/lockin-test-evil"),
        ("LOCKIN_TEST_OK", "1"),
    ]);
    let status = cmd
        .args(["env-var-unset", "DYLD_LIBRARY_PATH"])
        .status()
        .expect("run probe");
    assert!(
        status.success(),
        "DYLD_LIBRARY_PATH set via envs() must not reach child"
    );
}

#[test]
fn env_mutation_after_construction_is_stripped_at_spawn() {
    let probe = probe_binary();
    let mut cmd = sandbox_builder()
        .command(&probe)
        .expect("build sandbox command");
    cmd.env("LD_AUDIT", "/tmp/lockin-test-evil.so");
    let status = cmd
        .args(["env-var-unset", "LD_AUDIT"])
        .status()
        .expect("run probe");
    assert!(
        status.success(),
        "LD_AUDIT set via SandboxedCommand::env must not reach the child"
    );
}
