//! Build-only assertion that every public type in scope §SP1
//! is reachable through `rafaello_core::supervisor` (c14).

use std::time::Duration;

use rafaello_core::supervisor::{
    PluginSupervisor, ShutdownReport, SpawnHandle, SpawnPaths, SupervisorConfig, RFL_BUS_FD_NUMBER,
};

#[test]
fn supervisor_public_surface_is_reachable() {
    let _: i32 = RFL_BUS_FD_NUMBER;
    assert_eq!(RFL_BUS_FD_NUMBER, 3);

    let cfg: SupervisorConfig = SupervisorConfig::default();
    assert_eq!(cfg.shutdown_grace, Duration::from_millis(200));
    assert_eq!(cfg.fittings_max_frame_bytes, 1 << 20);

    let _ = std::mem::size_of::<PluginSupervisor>();
    let _ = std::mem::size_of::<SpawnHandle>();
    let _ = std::mem::size_of::<SpawnPaths>();
    let _ = std::mem::size_of::<ShutdownReport>();

    let report = ShutdownReport::default();
    assert!(report.clean.is_empty());
    assert!(report.forced.is_empty());
    assert!(report.failed.is_empty());
}

#[cfg(feature = "test-fixture")]
#[test]
fn test_hooks_counters_start_at_zero() {
    use std::sync::atomic::Ordering;

    let acl = rafaello_core::broker_acl::BrokerAcl {
        plugins: std::collections::BTreeMap::new(),
        tool_routes: std::collections::BTreeMap::new(),
        frontends: std::collections::BTreeMap::new(),
    };
    let broker = rafaello_core::bus::Broker::new(acl).expect("empty ACL is valid");
    let sup = PluginSupervisor::new(broker, SupervisorConfig::default());
    let hooks = sup.test_hooks();
    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 0);
}
