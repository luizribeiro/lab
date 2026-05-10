//! c20 acceptance: dead-watch `WaitFailed` cached outcome takes the
//! SIGTERM → probe → SIGKILL path. Drives `shutdown_with_outcome`
//! directly with mock `signal_fn` / `probe_fn`.

use std::io;
use std::sync::{Arc, Mutex};

use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use rafaello_core::error::ReaperOutcome;
use rafaello_core::frontend::shutdown::shutdown_with_outcome;
use rafaello_core::frontend::FrontendConfig;

#[tokio::test]
async fn frontend_handle_shutdown_kills_on_waitfailed() {
    let cached = Some(Arc::new(ReaperOutcome::WaitFailed(io::Error::other(
        "synthetic",
    ))));
    let pid = Pid::from_raw(987_654);
    let config = FrontendConfig {
        shutdown_grace: std::time::Duration::from_millis(10),
        shutdown_kill_grace: std::time::Duration::from_millis(10),
        ..FrontendConfig::default()
    };
    let (_tx, rx) = tokio::sync::watch::channel(None);

    let signals: Arc<Mutex<Vec<Signal>>> = Arc::new(Mutex::new(Vec::new()));
    let probes: Arc<Mutex<Vec<Pid>>> = Arc::new(Mutex::new(Vec::new()));

    let signals_clone = Arc::clone(&signals);
    let signal_fn = move |_pid: Pid, sig: Signal| -> Result<(), Errno> {
        signals_clone.lock().unwrap().push(sig);
        Ok(())
    };
    let probes_clone = Arc::clone(&probes);
    // First probe: alive (Ok). Subsequent: ESRCH.
    let probe_fn = move |p: Pid| -> Result<(), Errno> {
        let mut g = probes_clone.lock().unwrap();
        g.push(p);
        if g.len() == 1 {
            Ok(())
        } else {
            Err(Errno::ESRCH)
        }
    };

    let report =
        shutdown_with_outcome(cached, pid, &config, rx, None, None, signal_fn, probe_fn).await;

    assert!(report.used_sigterm, "WaitFailed branch must SIGTERM");
    assert!(report.used_sigkill, "alive after SIGTERM => SIGKILL");
    assert_eq!(
        *signals.lock().unwrap(),
        vec![Signal::SIGTERM, Signal::SIGKILL]
    );
    assert!(report.exit_status.is_none());
}
