use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use capsa_sandbox::{FdRemap, SandboxSpec};

#[derive(Debug, Clone, Copy)]
pub struct DaemonBinaryInfo {
    pub daemon_name: &'static str,
    pub binary_name: &'static str,
    pub env_override: &'static str,
}

#[derive(Debug)]
pub struct DaemonSpawnSpec {
    pub args: Vec<String>,
    pub sandbox: SandboxSpec,
    pub fd_remaps: Vec<FdRemap>,
    /// When true, daemon stdin is detached from caller TTY (`/dev/null`).
    pub stdin_null: bool,
}

pub trait DaemonReadiness {
    /// One-shot readiness barrier; consuming self is intentional.
    fn wait_ready(self, timeout: Duration) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct NoReadiness;

impl DaemonReadiness for NoReadiness {
    fn wait_ready(self, _timeout: Duration) -> Result<()> {
        Ok(())
    }
}

pub trait DaemonAdapter: Send + Sync + 'static {
    type Spec: serde::Serialize + std::fmt::Debug + Send + Sync + 'static;
    type Handoff: Send + 'static;
    type Ready: DaemonReadiness + Send + 'static;

    fn binary_info() -> DaemonBinaryInfo;

    fn spawn_spec(
        spec: &Self::Spec,
        handoff: &mut Self::Handoff,
        binary_path: &Path,
    ) -> Result<DaemonSpawnSpec>;

    fn readiness(spec: &Self::Spec, handoff: &mut Self::Handoff) -> Result<Self::Ready>;

    fn on_spawned(spec: &Self::Spec, handoff: &mut Self::Handoff) -> Result<()>;

    fn on_spawn_failed(spec: &Self::Spec, handoff: Self::Handoff) -> Result<()>;

    fn on_shutdown(spec: &Self::Spec, handoff: Self::Handoff) -> Result<()>;
}
