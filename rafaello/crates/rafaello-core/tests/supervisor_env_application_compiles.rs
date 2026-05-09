//! c18 SP4 Phase B step 12 — env application is build-only at this commit
//! (env behaviour is unobservable without a fixture's `dump_env` service —
//! c20+; behavioural verification is c27). Referencing
//! `PluginSupervisor::spawn` from an integration test is sufficient to
//! prove the env-application code path lives in the supervisor: integration
//! tests link against the library, so a build failure in the
//! env-application code would prevent this file from compiling.

#![cfg(feature = "test-fixture")]

use rafaello_core::compile::EnvPlan;
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};

#[test]
fn env_application_compiles() {
    // Take a function pointer to spawn; if env-application code in the
    // function body fails to compile, this file won't link.
    let _spawn_ref = PluginSupervisor::spawn;
    let _env: EnvPlan = EnvPlan::default();
    let _config: SupervisorConfig = SupervisorConfig::default();
    let _paths_ty: Option<SpawnPaths> = None;
}
