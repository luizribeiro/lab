#![cfg(target_os = "macos")]
//! Darwin-only Mach service contract tests.
//!
//! Mach IPC is a macOS-specific concept; Linux has no equivalent. These
//! tests verify lockin's contract about which Apple Mach services the
//! imported `system.sb` baseline allows, and that the M2a hardening
//! denies (`(deny mach-register (local-name-prefix ""))` and
//! `(deny mach-lookup (xpc-service-name-prefix ""))`) hold for
//! out-of-baseline names.
//!
//! See `docs/macos-seatbelt-baseline.md` §3.5 for the full list of
//! Mach services the baseline allows via `(global-name ...)`.

mod common;

use common::run_probe;

/// Positive control: an Apple service that the system.sb baseline
/// explicitly allows must be reachable from inside lockin's sandbox.
/// Without this passing, every "denied" assertion below would be
/// vacuous (the test machinery could be silently always-failing).
///
/// Service chosen: `com.apple.system.notification_center` (Darwin
/// notification center). It is listed at system.sb:172 in the audit
/// (§3.5) and is shipped on every macOS release; it is the most
/// universally-present Mach service in the baseline.
#[test]
fn baseline_allowed_mach_service_is_reachable() {
    assert!(
        run_probe(
            common::sandbox_builder(),
            &["can-mach-lookup", "com.apple.system.notification_center"]
        ),
        "baseline-allowed Mach service must be reachable; if this \
         fails the probe machinery is broken and the negative tests \
         below are meaningless"
    );
}

/// M2a's `(deny mach-register (local-name-prefix ""))` should block
/// any attempt to register a Mach name in the bootstrap namespace.
/// Outside the sandbox `bootstrap_register` for an arbitrary name
/// succeeds (verified during probe development), so a failure inside
/// the sandbox is attributable to the policy, not to launchd refusing
/// the call for unrelated reasons.
#[test]
fn mach_register_blanket_deny_blocks_arbitrary_name() {
    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-mach-register", "com.lockin.test.register"]
    ));
}

/// M2a's `(deny mach-lookup (xpc-service-name-prefix ""))` should
/// block XPC-API lookups of arbitrary service names. The probe goes
/// through `xpc_connection_create_mach_service` +
/// `xpc_connection_send_message_with_reply_sync`, which is the path
/// that triggers the `xpc-service-name` filter (a regular
/// `bootstrap_look_up` only matches `global-name`).
///
/// The reply is expected to be an `XPC_TYPE_ERROR` object; the probe
/// exits nonzero on that outcome regardless of whether the underlying
/// kernel reason is "sandbox denied" or "no such service registered".
/// Combined with the positive control above, a deny here is evidence
/// the M2a XPC deny is in effect.
#[test]
fn xpc_service_lookup_blanket_deny_blocks_arbitrary_name() {
    assert!(!run_probe(
        common::sandbox_builder(),
        &["can-xpc-lookup", "com.lockin.test.xpc"]
    ));
}
