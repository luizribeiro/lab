use std::path::PathBuf;

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

pub fn run_probe(spec: &capsa_sandbox::SandboxSpec, args: &[&str]) -> bool {
    let probe = probe_binary();
    let sandbox = capsa_sandbox::Sandbox::new(spec.clone())
        .unwrap_or_else(|e| panic!("failed to build sandbox for probe {}: {e}", probe.display()));

    let status = sandbox
        .command(&probe)
        .args(args)
        .status()
        .unwrap_or_else(|e| {
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
