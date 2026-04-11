pub use lockin_testkit::{spawn_drain, ChildGuard};

/// Returns a [`lockin::SandboxBuilder`] with both `LOCKIN_*` and
/// `CAPSA_*` environment variables applied.
///
/// `LOCKIN_*` vars provide base library paths for any sandboxed
/// binary; `CAPSA_*` vars add capsa-specific paths (e.g. libkrun).
pub fn sandbox_builder() -> lockin::SandboxBuilder {
    let mut builder = lockin_testkit::sandbox_builder();
    if let Some(val) = std::env::var_os("CAPSA_SYD_PATH") {
        builder = builder.syd_path(std::path::PathBuf::from(val));
    }
    if let Some(val) = std::env::var_os("CAPSA_LIBRARY_DIRS") {
        for dir in std::env::split_paths(&val) {
            if !dir.as_os_str().is_empty() {
                builder = builder.library_path(dir);
            }
        }
    }
    builder
}
