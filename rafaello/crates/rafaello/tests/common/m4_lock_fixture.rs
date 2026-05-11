//! Shared fixture for m4 `rfl chat` tests: materialise a
//! `rafaello.lock` with a real mockprovider + readfile install so
//! `run_chat` reaches the TUI flow. Tests that need an
//! intentionally provider-less lock use [`write_empty_lock`].

use std::path::Path;

use super::m4_install::{install_demo_layout, InstallOptions};

#[allow(dead_code)]
pub fn write_stub_lock(dir: &Path) {
    install_demo_layout(
        dir,
        InstallOptions {
            provider_executable: true,
            tool_executable: true,
            real_binaries: true,
        },
    );
}

#[allow(dead_code)]
pub fn write_empty_lock(dir: &Path) {
    let lock_path = dir.join("rafaello.lock");
    std::fs::write(&lock_path, "").expect("write empty rafaello.lock");
}
