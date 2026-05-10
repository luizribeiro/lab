use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cur: &Path = manifest_dir.as_path();
    loop {
        let candidate = cur.join("Cargo.toml");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if text.contains("[workspace]") {
                    return cur.to_path_buf();
                }
            }
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => panic!(
                "could not locate workspace root walking up from {}",
                manifest_dir.display()
            ),
        }
    }
}

fn target_dir() -> PathBuf {
    if let Some(t) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(t);
    }
    workspace_root().join("target")
}

fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

#[allow(dead_code)]
pub fn workspace_bin(name: &str) -> PathBuf {
    let path = target_dir().join(profile()).join(name);
    if !path.is_file() {
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--workspace",
                "--bins",
                "--features",
                "rafaello-core/test-fixture",
            ])
            .current_dir(workspace_root())
            .status()
            .expect("cargo build invocation failed");
        assert!(status.success(), "cargo build --workspace --bins failed");
    }
    assert!(
        path.is_file(),
        "expected workspace bin at {}",
        path.display()
    );
    path
}
