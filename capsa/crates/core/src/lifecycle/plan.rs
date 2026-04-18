//! Small helpers shared by `netd.rs`, `network.rs`, and `vmm.rs`:
//! the guest-side fd + MAC carried through `spawn_vmm`, and the
//! path canonicalization used by every sandbox builder.

use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

/// The guest-side socket fd and MAC for one interface after netd
/// has consumed the host end.
pub(super) struct VmmInterfaceBinding {
    pub(super) mac: [u8; 6],
    pub(super) guest_fd: OwnedFd,
}

/// Resolves symlinks before a path is handed across a process
/// boundary so capsa-vmm and the darwin sandbox-exec policy
/// agree on the post-resolution path the kernel will see at
/// `open(2)` time. Falls back to the input for paths that
/// don't exist (test fixtures, etc.).
pub(super) fn canonical_or_unchanged(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
