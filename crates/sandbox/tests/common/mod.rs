// Individual test binaries compile this shared module in isolation and may
// not exercise every item, so silence the resulting dead-code noise.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Child;

use capsa_sandbox::SandboxBuilder;

/// RAII wrapper that SIGKILLs and reaps a spawned child on drop, so a
/// panicking `#[test]` body never leaks a subprocess.
pub struct ChildGuard(pub Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub struct TestDir {
    dir: tempfile::TempDir,
}

impl TestDir {
    pub fn new(prefix: &str) -> Self {
        let base = std::env::temp_dir().join("capsa-sandbox-tests");
        std::fs::create_dir_all(&base)
            .unwrap_or_else(|e| panic!("failed to create test base dir {}: {e}", base.display()));

        let dir = tempfile::Builder::new()
            .prefix(&format!("{prefix}-"))
            .tempdir_in(&base)
            .unwrap_or_else(|e| panic!("failed to create test dir under {}: {e}", base.display()));

        Self { dir }
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }
}

/// Runs the sandbox probe with `args` under the sandbox configured by
/// `builder`. The builder is consumed; callers should construct a fresh
/// one per invocation.
pub fn run_probe(builder: SandboxBuilder, args: &[&str]) -> bool {
    let probe = probe_binary();
    let (mut command, _sandbox) = builder
        .build(&probe)
        .unwrap_or_else(|e| panic!("failed to build sandbox for probe {}: {e}", probe.display()));

    let status = command.args(args).status().unwrap_or_else(|e| {
        panic!(
            "failed to run sandboxed probe {} with args {:?}: {e}",
            probe.display(),
            args
        )
    });

    status.success()
}

pub fn probe_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_sandbox_probe") {
        return PathBuf::from(path);
    }

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/sandbox_probe");

    if cfg!(target_os = "windows") {
        path.set_extension("exe");
    }

    assert!(
        path.exists(),
        "sandbox probe binary not found at {}",
        path.display()
    );

    path.canonicalize()
        .unwrap_or_else(|e| panic!("failed to canonicalize {}: {e}", path.display()))
}
