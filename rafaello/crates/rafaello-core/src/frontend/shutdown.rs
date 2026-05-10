//! Pure shutdown algorithm for [`crate::frontend::FrontendHandle`]
//! (scope §F4 + §"shutdown_with_outcome").

use std::sync::Arc;
use std::time::Instant;

use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::unistd::Pid;

use crate::bus::RegisteredFrontend;
use crate::error::ReaperOutcome;
use crate::frontend::{FrontendConfig, ShutdownReport};

#[allow(clippy::too_many_arguments)]
pub async fn shutdown_with_outcome(
    cached: Option<Arc<ReaperOutcome>>,
    child_pid: Pid,
    config: &FrontendConfig,
    mut reaper_outcome_rx: tokio::sync::watch::Receiver<Option<Arc<ReaperOutcome>>>,
    serve_handle: Option<tokio::task::JoinHandle<()>>,
    register_guard: Option<RegisteredFrontend>,
    mut signal_fn: impl FnMut(Pid, Signal) -> Result<(), Errno>,
    mut probe_fn: impl FnMut(Pid) -> Result<(), Errno>,
) -> ShutdownReport {
    let start = Instant::now();
    let serve_was_some = serve_handle.is_some();

    let mut used_sigterm = false;
    let mut used_sigkill = false;
    let mut exit_status: Option<std::process::ExitStatus> = None;

    match cached.as_deref() {
        Some(ReaperOutcome::Exited(status)) => {
            exit_status = Some(*status);
        }
        Some(ReaperOutcome::WaitFailed(_)) | Some(ReaperOutcome::ReaperPanicked) => {
            used_sigterm = true;
            let _ = signal_fn(child_pid, Signal::SIGTERM);
            tokio::time::sleep(config.shutdown_grace).await;
            match probe_fn(child_pid) {
                Err(Errno::ESRCH) => {}
                _ => {
                    used_sigkill = true;
                    let _ = signal_fn(child_pid, Signal::SIGKILL);
                    tokio::time::sleep(config.shutdown_kill_grace).await;
                    if let Err(e) = probe_fn(child_pid) {
                        if e != Errno::ESRCH {
                            tracing::warn!(
                                pid = child_pid.as_raw(),
                                errno = %e,
                                "post-SIGKILL probe failed",
                            );
                        }
                    }
                }
            }
        }
        None => {
            used_sigterm = true;
            let _ = signal_fn(child_pid, Signal::SIGTERM);
            let _ = tokio::time::timeout(config.shutdown_grace, reaper_outcome_rx.changed()).await;
            let after_term = reaper_outcome_rx.borrow().clone();
            match after_term.as_deref() {
                Some(ReaperOutcome::Exited(status)) => {
                    exit_status = Some(*status);
                }
                _ => {
                    used_sigkill = true;
                    let _ = signal_fn(child_pid, Signal::SIGKILL);
                    let _ = tokio::time::timeout(
                        config.shutdown_kill_grace,
                        reaper_outcome_rx.changed(),
                    )
                    .await;
                    if let Some(ReaperOutcome::Exited(status)) =
                        reaper_outcome_rx.borrow().as_deref()
                    {
                        exit_status = Some(*status);
                    }
                }
            }
        }
    }

    if let Some(handle) = serve_handle {
        handle.abort();
        let _ = handle.await;
    }
    drop(register_guard);

    ShutdownReport {
        exit_status,
        used_sigterm,
        used_sigkill,
        serve_aborted: serve_was_some,
        elapsed: start.elapsed(),
    }
}
