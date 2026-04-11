// Privilege hardening (NO_NEW_PRIVS, capability dropping) is managed
// entirely by the sandbox backends: syd on Linux, sandbox-exec on
// macOS. There are no user-facing builder knobs for these features,
// so there is nothing to test at the lockin API level.
//
// Backend-level coverage lives in the syd / sandbox-exec integration
// tests (fd_inheritance, filesystem, network, resource_limits).
