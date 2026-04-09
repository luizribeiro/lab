// Re-export shim: the canonical types live in `capsa-spec`. This shim
// exists only so existing `crate::daemon::net::spec::...` imports keep
// working during the migration. Commits 4/5 switch daemon binaries to
// depend on `capsa-spec` directly and this shim can be deleted.
pub use capsa_spec::{NetInterfaceSpec, NetLaunchSpec};
