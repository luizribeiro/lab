use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmConfig {
    pub kernel: PathBuf,
    pub initramfs: Option<PathBuf>,
    pub kernel_cmdline: Option<String>,
    pub vcpus: u8,
    pub memory_mib: u32,
}
