# Runtime extensibility discussion

## Verdict

I broadly agree with the m2 direction: keep the runtime concrete around lockin, outpost-proxy, fittings, and inherited socketpairs rather than introducing a generic "runtime backend" abstraction now. The important extensibility seam is already in the right place: the broker sees authenticated principals plus `PeerHandle`s, while the supervisor owns how a process is created and how the fd/transport becomes a peer. I would push back only if the argument is "therefore no extensibility work is needed" — there are a few cheap names and comments worth tightening so m2 does not accidentally turn lockin implementation details into the long-term plugin ABI.

## What you got right

The broker/transport split in overview §4.1 is the load-bearing future-proofing. Once a connection is registered, the broker should not know whether the peer came from a lockin process, a VM shim, an externally attached frontend, or a future helper path. `Broker::register_plugin(canonical, PeerHandle)` is therefore a much better boundary than anything fd-shaped or sandbox-shaped. That is the abstraction to preserve; a new `Runtime` trait in m2 would mostly duplicate it badly.

The process model in §3 also points in the right direction: authenticate the principal at spawn/attach time, then derive publish authority from that connection identity. That is stronger and more extensible than letting a plugin self-identify in message bodies. It also means a future capsa/VM backend does not need to mimic every lockin detail; it only needs to deliver a broker-owned peer whose identity core assigned.

The current lockin API is not pretending to be backend-neutral, and that is good. It exposes a concrete builder, concrete filesystem grants, concrete `NetworkMode::{Deny, AllowAll, Proxy}`, fd inheritance via `inherit_fd_as`, and a concrete tokio child wrapper. m2 depending on that directly is honest. Hiding it behind a trait today would produce a leaky lowest-common-denominator interface before we have a second backend to test it against.

Outpost is the one place where sharing *is* already real. `outpost::NetworkPolicy` is backend-neutral vocabulary; `outpost-proxy` is a lockin-backend enforcement mechanism. m2's use of `NetworkPolicy::from_allowed_hosts(...)` for validation is the right level of coupling. The plan should continue to describe network intent as host policy, not as "must run an HTTP CONNECT proxy" except inside the lockin spawn sequence.

## What I'd push back on

I would not design a `SandboxBackend` / `RuntimeBackend` trait in m2. The hard parts are not the method signatures; they are lifecycle semantics, peer authentication, fd/vsock/UDS bridging, signal and process-tree cleanup, temp/state directories, network enforcement, and error reporting. A trait written before capsa is actually integrated will either be too lockin-shaped to help or too abstract to enforce the invariants m2 cares about.

I would also be careful with the phrase "inherited fd" as if it were the architectural invariant. For v1 lockin, inherited `RFL_BUS_FD` is exactly right. For a VM backend, core may not literally inherit a Unix fd into the plugin process; there may be a guest shim, vsock, or host-side bridge. The invariant should be "core binds a connection to a principal it spawned/attached," not "every runtime can inherit fd N." The overview mostly says this correctly, but m2 scope naturally gets fd-heavy because lockin is the implementation.

The public error surface is a small future-extensibility wart. `SpawnError::Lockin` is accurate for m2, but if this enum becomes a stable-ish rafaello-core API, the name bakes the backend into callers' match arms. I do not think this is a blocker, and `#[non_exhaustive]` helps, but a neutral name like `SandboxBuild` or `SandboxPrepare` would age better while still carrying the lockin `anyhow::Error` internally.

I would not try to make frontend spawning fit the plugin supervisor too soon. Frontends are bus principals but not lockin-sandboxed plugins, and the TUI will want some of the same transport plumbing without the same grant/policy pipeline. Reusing low-level helper functions later is fine; making `PluginSupervisor` generic enough for frontends now would blur a useful trust-boundary distinction.

## What I'd add

I would explicitly name the future seam as "peer admission" rather than "runtime backend." The admission code takes whatever backend-specific spawn/attach mechanism exists and returns a registered principal plus `PeerHandle`. For m2 that mechanism is lockin + socketpair + `StdioTransport`; for v2 it could be external frontend attach; later it could be capsa. That terminology keeps attention on the security boundary instead of on process-launch plumbing.

I would preserve `CompiledPlugin` as policy intent, not lockin config. m2 already does this reasonably: the compiler emits filesystem/network/env/limit intent and the supervisor applies it to lockin. That distinction matters because capsa will not map one-to-one onto lockin's builder. The more the plan says "apply this intent using the lockin backend" rather than "the plan is a lockin plan," the less migration pain later.

I would document the temp-dir behavior as a backend-specific m2 deviation, not an ABI promise. Lockin's `env_clear()` currently removes `TMPDIR`/`TMP`/`TEMP`, and the tokio command wrapper does not expose the private tmp path before spawn. That is fine for m2 if documented, but future runtimes should not be forced to reproduce "no temp env vars." If plugins need a stable scratch contract, `RFL_PRIVATE_STATE_DIR` or a later `RFL_TEMP_DIR` is a better cross-runtime story.

I would keep outpost as the durable network-policy layer. Even if capsa eventually enforces at packet/VM boundary rather than with an HTTP CONNECT proxy, the manifest/lock/compiler should keep speaking in host policy terms. The current outpost docs already admit the hostname/DNS trust limitations; that is exactly the kind of semantic warning that should survive backend swaps.

## Cheap scope.md tweaks worth doing now (if any)

Add one short note near SP4 saying the lockin sequence is the v1 backend implementation, while the core invariant is principal-bound peer admission into the broker. That would prevent future readers from mistaking `RFL_BUS_FD` inheritance for the only acceptable architecture.

Consider renaming `SpawnError::Lockin` to a backend-neutral variant before implementation if no code depends on it yet. If you keep the current name, I would not fight it; just make sure the retrospective says this is an m2 concrete-backend error, not the shape of a future multi-runtime API.

Add a sentence under the outpost input/dependency bullets that `outpost::NetworkPolicy` is the shared policy vocabulary and `outpost-proxy` is only the lockin enforcement backend used by m2. That is a cheap clarification with high future value.

Add a sentence to the TMPDIR deviation saying it is not a plugin ABI guarantee. Plugins should use `RFL_PRIVATE_STATE_DIR` for durable scratch/state in m2; a portable temp-dir ABI can be added later if real plugins need it.
