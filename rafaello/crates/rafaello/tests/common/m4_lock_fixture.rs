//! Shared fixture for m4 `rfl chat` tests: materialise a minimal
//! stub `rafaello.lock` so `run_chat` proceeds past lock load.

use std::path::Path;

#[allow(dead_code)]
pub fn write_stub_lock(dir: &Path) {
    let lock_path = dir.join("rafaello.lock");
    std::fs::write(&lock_path, "").expect("write stub rafaello.lock");
}
