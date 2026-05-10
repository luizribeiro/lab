//! Dead-watch shutdown unit tests (scope §F4).

use std::io;
use std::sync::{Arc, Mutex};

use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::unistd::Pid;

use rafaello_core::error::ReaperOutcome;
use rafaello_core::frontend::shutdown::shutdown_with_outcome;
use rafaello_core::frontend::FrontendConfig;

const TEST_PID: i32 = 424242;

#[tokio::test]
async fn dead_watch_waitfailed_child_already_gone() {
    let cached = Some(Arc::new(ReaperOutcome::WaitFailed(io::Error::other(
        "boom",
    ))));
    let pid = Pid::from_raw(TEST_PID);
    let config = FrontendConfig::default();
    let (_tx, rx) = tokio::sync::watch::channel(None);

    let signals: Arc<Mutex<Vec<Signal>>> = Arc::new(Mutex::new(Vec::new()));
    let probes: Arc<Mutex<Vec<Pid>>> = Arc::new(Mutex::new(Vec::new()));

    let signals_for_fn = Arc::clone(&signals);
    let signal_fn = move |_pid: Pid, sig: Signal| -> Result<(), Errno> {
        signals_for_fn.lock().unwrap().push(sig);
        Ok(())
    };
    let probes_for_fn = Arc::clone(&probes);
    let probe_fn = move |p: Pid| -> Result<(), Errno> {
        probes_for_fn.lock().unwrap().push(p);
        Err(Errno::ESRCH)
    };

    let report =
        shutdown_with_outcome(cached, pid, &config, rx, None, None, signal_fn, probe_fn).await;

    assert!(report.used_sigterm);
    assert!(!report.used_sigkill);
    assert_eq!(report.exit_status, None);
    assert!(!report.serve_aborted);
    assert_eq!(*signals.lock().unwrap(), vec![Signal::SIGTERM]);
    assert_eq!(*probes.lock().unwrap(), vec![pid]);
}

#[tokio::test]
async fn dead_watch_reaper_panicked_child_alive() {
    let cached = Some(Arc::new(ReaperOutcome::ReaperPanicked));
    let pid = Pid::from_raw(TEST_PID);
    let config = FrontendConfig::default();
    let (_tx, rx) = tokio::sync::watch::channel(None);

    let signals: Arc<Mutex<Vec<Signal>>> = Arc::new(Mutex::new(Vec::new()));
    let probes: Arc<Mutex<Vec<Pid>>> = Arc::new(Mutex::new(Vec::new()));

    let signals_for_fn = Arc::clone(&signals);
    let signal_fn = move |_pid: Pid, sig: Signal| -> Result<(), Errno> {
        signals_for_fn.lock().unwrap().push(sig);
        Ok(())
    };
    let probes_for_fn = Arc::clone(&probes);
    let probe_fn = move |p: Pid| -> Result<(), Errno> {
        let mut guard = probes_for_fn.lock().unwrap();
        guard.push(p);
        if guard.len() == 1 {
            Ok(())
        } else {
            Err(Errno::ESRCH)
        }
    };

    let report =
        shutdown_with_outcome(cached, pid, &config, rx, None, None, signal_fn, probe_fn).await;

    assert!(report.used_sigterm);
    assert!(report.used_sigkill);
    assert_eq!(report.exit_status, None);
    assert!(!report.serve_aborted);
    assert_eq!(
        *signals.lock().unwrap(),
        vec![Signal::SIGTERM, Signal::SIGKILL]
    );
    assert_eq!(*probes.lock().unwrap(), vec![pid, pid]);
}
