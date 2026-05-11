//! Startup-ordering instrumentation seam (c02 / pi-3 M-4 / pi-5 B-2).
//!
//! `run_chat` records named events at well-known points in the
//! broker / supervisor / audit-writer wiring sequence so an
//! integration test can later assert ordering (e.g.
//! `set_audit_writer` precedes the first `PluginSupervisor::spawn`).
//! The queue is process-global, append-only, and drained by the test.
//! Memory cost in production is at most a handful of small enum
//! values per `run_chat` invocation.

use std::sync::OnceLock;

use parking_lot::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupEvent {
    SetAuditWriter,
    PluginSupervisorSpawn,
}

fn queue() -> &'static Mutex<Vec<StartupEvent>> {
    static Q: OnceLock<Mutex<Vec<StartupEvent>>> = OnceLock::new();
    Q.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn record(event: StartupEvent) {
    queue().lock().push(event);
    if let Some(path) = std::env::var_os("RFL_STARTUP_ORDERING_LOG") {
        let line = format!("{}\n", event.as_str());
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    }
}

impl StartupEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            StartupEvent::SetAuditWriter => "set_audit_writer",
            StartupEvent::PluginSupervisorSpawn => "plugin_supervisor_spawn",
        }
    }
}

pub fn drain() -> Vec<StartupEvent> {
    std::mem::take(&mut *queue().lock())
}

pub fn clear() {
    queue().lock().clear();
}
