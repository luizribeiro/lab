# Pi review round 1 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial, round 1. This document is intentionally combative and exhaustive. The goal is to flush out API mismatches, contradictions, missing security cases, and non-executable test plans before `commits.md` is drafted.

## Verdict

**Not ratifiable.**

The scope is ambitious and mostly aimed at the right runtime primitive, but it is currently full of build-breaking API mismatches, security invariants that are asserted but not actually enforceable, and test plans that cannot run as written.

The headline architecture — an in-process broker plus a lockin-backed supervisor using inherited socketpair authentication — is the right milestone shape. However, the current document cannot be handed to an implementer as-is. Several sections describe APIs that do not exist in the repository, several tests rely on Cargo/runtime behavior that is not true, and the broker authority algorithm has holes large enough to permit unauthorised namespaces.

The scope needs a substantial rewrite before round 2. In particular, the dependency coordinates, fittings construction model, broker namespace enforcement, event schema, supervisor handle API, spawn error surface, proxy env injection, shutdown semantics, and fixture strategy all need to be corrected.

## Things the scope gets right

These are worth preserving through the rewrite.

1. **The milestone boundary is directionally right.**
   - Broker + supervisor is the correct structural step after m1's pure compile/ACL work.
   - Keeping `rfl chat`, provider loop, frontend attach, session persistence, renderer model, confirmation, and tool dispatch out of m2 is the right cut.

2. **The identity principle is correct.**
   - The broker must authenticate publishers from the connection identity established at spawn time, not from message bodies.
   - The scope states this explicitly in the goal and in B3/SP3.

3. **The no-bypass spawn invariant is right and important.**
   - SP4's refusal to introduce `spawn_unsandboxed`, debug bypasses, or fixture-only escape paths is exactly the invariant reviewers should defend.
   - The fixture plugin being spawned through the same supervisor path is the correct testing posture.

4. **The broker/transport layering is correctly emphasised.**
   - The scope correctly names fittings as transport and rafaello-core as broker.
   - It avoids pushing topic/ACL/taint semantics into fittings.

5. **The test matrix is ambitious in the right direction.**
   - There are tests for namespace rejection, outside-grant rejection, invalid topic grammar, `in_reply_to`, self-fanout exclusion, spawn refusal, sandbox denial, reserved env, lifecycle, proxy, and env pass/set.
   - The matrix is not currently executable as written, but the behavioral coverage goals are mostly the right ones.

6. **The scope remembers runtime revalidation.**
   - Re-running topic grammar validation at publish time is necessary even though grants were validated at compile time.
   - This is a critical security boundary and should stay.

7. **The scope correctly treats m2 `in_reply_to` enforcement as a subset.**
   - m2 cannot validate real prior tool requests or RPC calls because those dispatch/correlation maps do not exist yet.
   - Enforcing presence and arity for `tool_result` and `rpc_reply` is a reasonable m2 subset if the deferral is explicit.

8. **The RAII registration idea is good.**
   - A `RegisteredPlugin` guard whose drop unregisters from the broker is the right shape for avoiding stale registry entries.
   - The idea needs sharper error semantics, but the ownership direction is sound.

9. **The scope recognises lockin's Rust builder API as the spawn-time target.**
   - This matches decisions row 32 and avoids inventing a `lockin.toml` artifact.

10. **The scope recognises that proxy mode needs an outpost proxy lifecycle.**
    - It misses env injection, but tying `NetworkPlan::Proxy` to `outpost_proxy::start` and lockin `network_proxy(port)` is the right structural composition.

11. **The scope includes manual validation, documentation build, and retrospective requirements.**
    - The manual validation list needs corrections, but requiring a `manual-validation.md`, `cargo doc --no-deps`, and `retrospective.md` is the right milestone hygiene.

## Blocking findings

These block ratification. They are either build-breakers, security-boundary failures, contradictions that make implementation ambiguous, or test plans that cannot execute.

### 1. Workspace dependency paths are wrong

Section: W1/W2.

The scope uses dependency paths that do not exist in the current repository:

- `../fittings/crates/fittings-core`
- `../fittings/crates/fittings-server`
- `../fittings/crates/fittings-client`
- `../fittings/crates/fittings-wire`

The actual crates are laid out as:

- `../fittings/crates/core` with package name `fittings-core`
- `../fittings/crates/server` with package name `fittings-server`
- `../fittings/crates/client` with package name `fittings-client`
- `../fittings/crates/wire` with package name `fittings-wire`

The lockin dependency is also wrong as written. W1 proposes:

```toml
lockin-sandbox = { path = "../lockin/crates/sandbox", package = "lockin-sandbox" }
```

But the actual package name in `lockin/crates/sandbox/Cargo.toml` is:

```toml
[package]
name = "lockin"
```

So the dependency should be on package `lockin`, likely renamed locally if desired:

```toml
lockin = { path = "../lockin/crates/sandbox" }
```

or:

```toml
lockin-sandbox = { path = "../lockin/crates/sandbox", package = "lockin" }
```

This is not cosmetic. The implementation cannot even resolve dependencies with W1 as written.

### 2. W1 omits dependencies the scoped implementation likely needs

Section: W1/W2/SP3/F3.

The scope adds fittings core/server/client/wire but not `fittings-transport`. Current fittings transport adapters live in `fittings/crates/transport`, package `fittings-transport`. The scoped plan talks about wrapping the inherited fd as a stream transport; there is no such transport in `fittings-core`, `fittings-server`, `fittings-client`, or `fittings-wire` alone.

The scope also does not add `async-trait`, even though every `fittings_core::service::Service` implementation requires `#[async_trait]` unless manually boxed.

Depending on final implementation, it may also need:

- `tokio-util` if cancellation tokens or fittings context internals are touched directly.
- `nix` features beyond `socket` and `fs` if real SIGTERM/SIGKILL lifecycle is implemented with nix signal APIs. Current W1 only enables `socket` and `fs`.

The current dependency list is not implementation-complete.

### 3. The plan misunderstands the current fittings architecture

Sections: SP3 steps 10-13, F3, overview interpretation.

The scope repeatedly says each side builds both a `Server` and a `Client` over the same inherited fd:

- SP3 step 10: "construct a `Server` + `Client` over it"
- F3: fixture uses fittings-core to build a `Client` and a `Server` on the same fd

That does not match the landed fittings API.

Current fittings already has bidirectional `PeerHandle` support inside a single connection object:

- Core side should likely be one `fittings_server::Server<Service, Transport>`.
- Plugin side should likely be one `fittings_client::Client<Connector>` with optional `with_service` for inbound peer-originated requests.

A `Server` already has `Server::peer() -> PeerHandle` for outbound notify/call. A `Client` already has `Client::peer() -> PeerHandle` and `Client::with_service(...)` for inbound service handling. Constructing both a server and a client over the same owned fd would require duplicating/splitting transport ownership in a way the current API does not support.

This is not merely wording. It affects the entire supervisor and fixture implementation sequence. The scope must rewrite SP3 and F3 around the real fittings API.

### 4. There is no scoped transport for inherited Unix fds

Sections: SP3 step 10, F3, W1.

The scope says:

> Build a fittings transport from `core_fd` (an `AsyncFd<UnixStream>` wrapper)

Current fittings has no Unix socket transport adapter. `fittings-transport::stdio::StdioTransport` is generic over `AsyncRead + AsyncWrite`, and could probably be reused with a split `tokio::net::UnixStream`, but the scope does not say that and does not include `fittings-transport` in W1.

`AsyncFd<UnixStream>` is also the wrong level if using Tokio's `UnixStream`; `tokio::net::UnixStream` already implements async read/write. The fixture and supervisor need a concrete, repo-real transport plan:

- convert `OwnedFd`/raw fd to `std::os::unix::net::UnixStream`,
- set nonblocking,
- convert to `tokio::net::UnixStream`,
- split read/write if using `StdioTransport`, or add a dedicated Unix transport adapter.

As written, SP3 handwaves the exact object fittings consumes.

### 5. Fixture binary discovery via `env!("CARGO_BIN_EXE_rfl-bus-fixture")` is not reliable for this layout

Sections: W3, F1, integration test harness.

The scope says the fixture is a separate workspace member and `rafaello-core` integration tests resolve it via:

```rust
env!("CARGO_BIN_EXE_rfl-bus-fixture")
```

Cargo does not generally set `CARGO_BIN_EXE_*` for arbitrary sibling workspace member binaries while compiling another package's integration tests. That environment variable is for binaries of the package whose tests Cargo is building.

Adding the fixture crate to `[workspace.members]` does not make `cargo test -p rafaello-core` build that binary or expose its path to `rafaello-core`'s tests.

This breaks the headline supervisor integration tests.

Possible fixes:

- Make the fixture a bin target of the `rafaello-core` package itself, so the env var is valid for its integration tests.
- Use a robust test helper that invokes Cargo metadata/builds the fixture explicitly and locates the binary in the workspace target dir.
- Change acceptance to run workspace-level tests and document the exact Cargo behavior being relied on, but this still needs proof.

The current W3 strategy should be considered invalid until demonstrated.

### 6. B8 is stale: the pattern matcher already exists

Sections: B7/B8.

The scope says m2 adds:

```rust
rafaello_core::validate::pattern::matches(pattern, topic)
```

because m1 only validated patterns syntactically.

Current m1 code already has:

```rust
rafaello_core::validate::topic::pattern_matches_topic(pattern, topic)
```

and it is re-exported from `validate/mod.rs`.

Adding a new `validate::pattern::matches` API would duplicate the existing function and split call sites. The scope should either:

- reuse `pattern_matches_topic`, or
- explicitly include a small refactor that renames/moves the existing helper while preserving a compatibility re-export if needed.

The current text is factually wrong against the repository.

### 7. Broker namespace enforcement has a fatal unknown-namespace hole

Section: B3.

B3 authorises a publish unless it matches one of the listed bad prefixes. The algorithm says:

1. reject `core.` / `provider.` / `frontend.`
2. reject `plugin.<X>.` where X is another plugin
3. reject own plugin namespace outside grant
4. otherwise authorised

This means a plugin can publish topics such as:

- `evil.namespace`
- `random.topic`
- `session.entry.appended`
- `foo.bar.baz`

Those topics pass the grammar (`segment.segment`) but are not in any of the four authorised namespaces. Decisions row 4 and security RFC §5.2 define exactly four top-level namespaces. Unknown top-level namespaces must be rejected.

This is a security-boundary bug in the core invariant.

The broker must parse the topic structurally and reject any top-level segment not in `{core, provider, plugin, frontend}`.

### 8. Broker namespace enforcement mishandles short plugin topics

Section: B3.

The B3 checks use `starts_with("plugin.<id>.")`. That misses grammar-valid two-segment plugin topics:

- `plugin.<own-topic-id>`
- `plugin.<other-topic-id>`

The canonical grammar requires at least two segments, so both are syntactically valid topics. They are probably semantically useless and should be rejected by ACL, but the current prefix algorithm lets them fall through to "otherwise authorised".

The same class of bug can appear with malformed assumptions around trailing dots. Security code should not use string-prefix checks as the sole namespace parser. It should split into segments and reason over top-level namespace and id segment explicitly.

### 9. B3 contradicts itself on `auto_subscribes` as publish authority

Section: B3 #4.

B3 says a plugin publishing under its own namespace is authorised if the topic is in:

```text
plugin_acl.publish_topics ∪ plugin_acl.auto_subscribes
```

Then the parenthetical says publishing on own auto-subscribe is a no-op nonsense case and:

> The conservative choice is to error; m2 takes it.

Those two statements are opposites.

`auto_subscribes` are subscribe grants. They are not publish grants. If m2 takes the conservative choice, then `auto_subscribes` must not be included in publish authority. Publishing `plugin.<own-topic-id>.tool_request` should return `PublishOutsideGrant` unless explicitly granted in `publish_topics`.

The current scope is internally contradictory on a security decision.

### 10. Provider namespace treatment conflicts with m1 ACL reality and future m4 semantics

Sections: B3, Out of scope provider plugins, Acceptance drift notes.

Current m1 validation and `BrokerAcl::PluginAcl` carry `provider_id: Option<String>`, and m1 lock validation allows provider plugins to publish `provider.<provider-id>.*` when `bindings.provider = true` and the provider id matches.

m2 scope says every plugin publish on `provider.*` is rejected because provider authority is deferred to m4.

That may be an acceptable milestone cut, but it is not framed sharply enough. There are two competing truths:

- Existing m1 ACL can contain provider publish grants.
- m2 broker says provider namespace is reserved and ignored until m4.

The scope must explicitly state whether m2 intentionally rejects even lock-granted provider publishes and how that interacts with `BrokerAcl.provider_id`. Otherwise implementers may naturally use `provider_id` and accidentally ship partial provider authority in m2.

This is especially dangerous because decisions row 6 says provider plugins publish on `provider.<provider-id>.*`, while m2 says provider plugins are out of scope. The scope must pin the temporary m2 behavior as a deliberate staged deviation.

### 11. `BrokerError::NotInAcl` is overloaded and wrong for live registration

Sections: B1/B2, broker_register_plugin test.

The scope says:

- `register_plugin` returns `NotInAcl` if the canonical is absent from the static `BrokerAcl`.
- Dropping the registration guard unregisters the plugin.
- The test then expects `handle_plugin_publish` after drop to return `NotInAcl`.

But after dropping the guard, the canonical is still in the ACL. It is not live-registered.

These are different states:

1. Static ACL does not contain canonical.
2. Static ACL contains canonical but there is no live registered peer.

The error enum needs a distinct variant such as:

```rust
NotRegistered(CanonicalId)
```

or the semantics of `NotInAcl` need to be renamed and broadened. As written, the test asserts a lie about the ACL.

### 12. `PublishMsg` decoding errors cannot be produced by the scoped broker API

Sections: B1/B2/B4/SP3 step 11.

B1 defines:

```rust
Broker::handle_plugin_publish(canonical: &CanonicalId, msg: PublishMsg)
```

B2 says `BrokerError::InvalidPayload` covers params decode failure.

But if the function receives a typed `PublishMsg`, serde decoding has already succeeded. Unknown-field rejection and shape errors happen before this function is called.

The scope needs to choose where payload decoding lives:

- Broker API accepts raw `Value` params and decodes to `PublishMsg`, so it can return `InvalidPayload`.
- Or the fittings `Service` layer decodes params and maps decode errors into broker lifecycle rejection events.

Right now the error surface promises behavior the function signature cannot implement.

### 13. Outbound `BusEvent` is used but never defined

Sections: B1/B7/Demo bar tests.

B7 says fan-out calls:

```rust
peer.notify("bus.event", BusEvent { topic, payload, in_reply_to, taint })
```

But B1 never defines `BusEvent` as a public or private wire type. It defines only `PublishMsg` and `TaintEntry`.

The headline test also says it asserts "that the publish authority was the fixture's canonical id." There is no field in the sketched `BusEvent` carrying publisher identity.

The security RFC says the broker tags every event at publish time with the publisher's plugin id taken from the authenticated connection. The m2 event schema needs to decide whether that tag is included in the delivered params. If tests require it, the type must include it, e.g.:

```rust
pub struct BusEvent {
    pub topic: String,
    pub payload: Value,
    pub publisher: PublisherIdentity,
    pub in_reply_to: Option<Vec<JsonRpcId>>,
    pub taint: Option<Vec<TaintEntry>>,
}
```

or similar.

As written, B7 references a missing type and the tests assert a missing field.

### 14. Security RFC result-routing is violated by generic fan-out

Sections: B7, Out of scope tool dispatch, security RFC §5.4.1.

Security RFC §5.4.1 says a plugin's raw result event:

```text
plugin.<tool-id>.tool_result
```

is not delivered directly to other plugins or providers. Core validates it and re-emits:

```text
core.session.tool_result
```

Only the canonical core event reaches subscribers.

B7's generic fan-out delivers every topic to every matching subscriber, including `plugin.<id>.tool_result`. That bypasses the canonical re-emission model.

The scope says m2 has no tool dispatch and m4 owns full validation. That is fine, but then m2 must not accidentally create a direct delivery path that later milestones have to break. Options:

- Explicitly special-case `plugin.<id>.tool_result` and `plugin.<id>.rpc_reply` as publish-accepted but not fan-out-delivered except to core-internal handlers.
- Declare generic fan-out as an m2-only test primitive and explicitly require m4 to change it, though this is dangerous because it bakes the wrong behavior into integration tests.
- Scope m2 to fan out non-result plugin topics only.

The current B7 contradicts the security RFC's result-routing invariant.

### 15. `core.lifecycle.publish_rejected` can recurse or fail silently without a defined path

Sections: B3/B7/Out of scope lifecycle rejection events.

The scope says broker rejection emits:

```text
core.lifecycle.publish_rejected
```

But it does not define whether that emission uses `publish_core`, an internal unchecked fan-out path, or some special rejection path. If it uses `publish_core`, errors can occur during error reporting. If it uses the same fan-out machinery, what happens if subscribers reject, drop, or panic? If it includes invalid original topic strings in payload, what exact schema?

This event class is explicitly not in the security RFC yet. That is acceptable only if the m2 scope defines the schema and recursion/error behavior tightly.

Currently it is a sentence, not a contract.

### 16. `publish_core` lacks authority semantics beyond `core.` prefix

Section: B1/B5/B7.

`Broker::publish_core(topic, payload)` requires `topic` starts with `core.` and validates grammar. That rejects non-core topics from core.

But core may eventually need to route by publishing plugin request topics (`plugin.<id>.tool_request`) per security RFC §5.4, and provider observation/core re-emission may need internal broker paths. m2 can defer those paths, but the API name `publish_core` risks conflating "core as publisher" with "core namespace only".

If m2 intentionally only exposes core namespace publish, the function should be named or documented accordingly, e.g. `publish_core_event`. Otherwise later m4 will need either a new internal routing API or a breaking semantic expansion.

This is a design blocker because the m2 API shape will be public in `rafaello-core`.

### 17. Supervisor API cannot support its own multi-spawn tests

Sections: SP1, positive integration tests.

SP1 proposes:

```rust
PluginSupervisor::spawn(&mut self, plan: &CompiledPlugin) -> Result<&SpawnedPlugin, SpawnError>
```

This API is hostile to multi-plugin tests and runtime use. If caller keeps the returned `&SpawnedPlugin`, the mutable borrow of the supervisor prevents calling `spawn` again.

But the test matrix includes:

- observer fixture plus publishing fixture,
- two-fixture topology,
- multi-plugin fan-out.

Returning a borrowed reference makes those tests awkward or impossible without immediately copying fields and dropping the borrow. It also makes handle lifetime unclear.

Use one of these instead:

- `Result<SpawnHandle, SpawnError>` where the supervisor stores shared state internally,
- `Result<Arc<SpawnedPlugin>, SpawnError>`,
- `Result<PluginId/Index, SpawnError>` plus lookup APIs,
- or `Result<SpawnedPluginHandle, SpawnError>` with RAII retained internally.

The current API shape contradicts the demo bar.

### 18. Supervisor has no API to wait for child exit or inspect exit status

Sections: SP1, positive test `supervisor_peer_call_plugin_to_core.rs`, fixture behavior.

The test matrix says:

> The test waits for the fixture to exit 0.

But `SpawnedPlugin` exposes only:

- `canonical`
- `topic_id`
- `child_pid`
- `peer`

A PID is not enough to obtain an exit status. Polling the process table cannot recover whether it exited 0 or 2. The supervisor either needs:

- `wait()` / `try_wait()` on `SpawnedPlugin`,
- an exit-status channel,
- a `JoinHandle` for a reaper task,
- or a test-only side channel.

As written, the test cannot assert what it says it asserts.

### 19. `SpawnError` enum is incomplete and references nonexistent types

Sections: SP2/SP6.

Problems:

1. SP6 says reserved env violations return:

   ```rust
   SpawnError::Internal { detail: ... }
   ```

   But SP2 does not define `Internal`.

2. `broker.register_plugin` can return `AlreadyRegistered`, but SP2 has only `NotInAcl` for broker registration failure.

3. SP2 references:

   ```rust
   lockin_sandbox::Error
   ```

   Actual lockin sandbox crate package is `lockin`, and `SandboxBuilder::command` returns `anyhow::Result<SandboxedCommand>`. There is no `lockin_sandbox::Error` in the current API.

4. SP2 references:

   ```rust
   fittings_core::Error
   ```

   The current type is `fittings_core::error::FittingsError`.

5. `outpost_proxy::start` returns `std::io::Result<ProxyHandle>`, so `ProxyStart` source is fine, but the scope should be precise that only proxy startup errors use `std::io::Error`.

The error surface is not implementable as written.

### 20. Spawn sequencing claims broker registration happens before child can publish, but the order does the opposite

Sections: Goal, B1, SP3.

The goal says:

> Broker::register_plugin ... called by the supervisor immediately after spawn (before the child has had a chance to publish anything; the spawn sequencing is §SP3).

But SP3 does:

8. spawn child
9. drop child fd parent side
10. build transport
11. build service
12. register plugin
13. spawn serve loop

After step 8, the child can run immediately and write `bus.publish` to fd 3 before the core has built the transport, registered the plugin, or started serving. The socket buffer may hold the frame, but the broker has not accepted/registerd the connection before the child publishes.

This is not necessarily fatal if the core side reads only after registration, but the current wording is false. If any part of fittings starts reading before registration, early messages could observe an unregistered state. If nothing reads until after registration, then the invariant should be restated as:

> the core does not process inbound frames until after registration

not:

> before the child has had a chance to publish.

The spawn sequence needs a precise race contract.

### 21. There is no handshake/readiness story

Sections: SP3/F2/F3/integration tests.

The fixture publishes on startup. Tests then expect observer fixtures to receive events. But there is no defined readiness signal that:

- the core serve loop is running,
- the broker registration is installed,
- the observer fixture has registered and is ready to receive `bus.event`,
- the publishing fixture has not already published before observer registration.

The test harness says it adds an observer fixture subscribed to `**`, but does not define spawn order or readiness. Without a handshake, the headline publish-on-startup test is race-prone.

Possible fixes:

- Fixture waits for a core `core.fixture.start` call before publishing.
- Supervisor spawn returns only after a ping/pong over the fittings peer succeeds.
- Harness spawns observer first and waits for a `ready` side-channel call.

As written, the integration tests are schedule-dependent.

### 22. `CompiledPlugin` does not prove correspondence to a lock entry

Sections: Goal, SP3.1, Risks #7.

The goal says:

> The supervisor refuses to spawn anything that does not correspond to a lock entry — there is no hardcoded bypass path.

SP3.1 checks only that `plan.canonical` is in `broker.acl.plugins`.

But `CompiledPlugin` has public fields. A caller can take a valid compiled plan and mutate:

- `entry_absolute`
- filesystem grants
- network plan
- env
- limits
- subscribe/publish lists
- provider id

The canonical still exists in the ACL, so SP3.1 passes. The supervisor then spawns something that does not correspond to the lock entry.

The scope later admits in Risks #7 that test code can construct invalid `CompiledPlugin` values and the supervisor mostly trusts compile output. That contradicts the strong goal claim.

Either weaken the goal to "production code passes compiler output; supervisor spot-checks only selected invariants" or redesign the API to take an unforgeable plan generated from a lock/ACL pair.

### 23. Reserved env var story is inconsistent with m1 code

Sections: SP6, m1 scrubber.

SP6 says m1 already strips/rejects:

- `RFL_BUS_FD`
- `RFL_PLUGIN`
- `RFL_HELPER_FD`

Current `scrubber.rs` reserves only:

```rust
const RESERVED_ENV_VARS: &[&str] = &["RFL_BUS_FD", "RFL_PLUGIN"];
```

`RFL_HELPER_FD` is not rejected by m1 code.

SP6 can still add a defense-in-depth spawn-time check for all three names, but it must not claim m1 already rejected the helper var. If m2 wants `RFL_HELPER_FD` protected before helpers exist, either:

- include a small m2 change to m1 scrubber reserved list, or
- accurately state that m2 only catches hand-constructed/mutated plans at spawn time.

### 24. `inherit_fd_as` ownership text is contradictory

Sections: SP3 steps 5 and 9.

Lockin's actual API is:

```rust
pub fn inherit_fd_as(mut self, fd: OwnedFd, child_fd: RawFd) -> Self
```

It consumes the `OwnedFd`.

SP3 step 5 passes `child_fd` into `inherit_fd_as(child_fd, 3)`. SP3 step 9 then says:

> Drop `child_fd` on the parent side. It was passed by `inherit_fd_as` (which takes ownership of the fd internally — verify against lockin's `OwnedFd` API).

Once the `OwnedFd` has been moved into `inherit_fd_as`, there is no `child_fd` left to drop. The parent may drop the builder/command or let it transfer into the child, but the local variable is gone.

This needs to be rewritten to match Rust ownership and lockin semantics.

### 25. Entry executable check is underspecified and maybe impossible portably as stated

Sections: SP2/SP3 step 6.

The scope wants `EntryNotExecutable` before `SandboxBuilder::command` is consumed.

It does not define:

- Unix executable bit check details.
- Symlink handling.
- Directory handling beyond "executable file".
- macOS skip behavior.
- Whether scripts require interpreter read/exec access.
- Whether Nix store executables behind symlinks are canonicalized.

m1 compile already ensures entry exists and is a regular file, but not necessarily executable. If m2 adds a pre-spawn exec-bit check, define it precisely for Unix:

- use `std::os::unix::fs::PermissionsExt::mode() & 0o111 != 0`, perhaps after metadata following symlink;
- reject non-files;
- state Windows is unsupported or cfg-unix only.

Otherwise tests will encode accidental behavior.

### 26. Lockin proxy mode is incomplete without proxy env injection

Sections: SP3 step 3/4/7, positive proxy test.

The scope starts `outpost_proxy::start(policy)` and configures lockin:

```rust
network_proxy(loopback_port)
```

But lockin's CLI proxy path also injects env vars into the child:

- `HTTP_PROXY`
- `HTTPS_PROXY`
- `http_proxy`
- `https_proxy`
- `ALL_PROXY`
- `all_proxy`
- `NO_PROXY` = empty
- `no_proxy` = empty

Without these env vars, most HTTP clients in the plugin will not use the proxy. Lockin will allow only the loopback proxy port, but the child will not know to connect to it. The result is fail-closed network, not functional proxy mode.

m2's proxy smoke test only asserts proxy startup and port plumbing, so it would not catch this missing behavior. The supervisor must either inject the proxy env pairs or explicitly defer functional proxy mode and weaken the claim.

Given the milestone goal includes locked plugin spawn with network policy, missing proxy env is a blocker.

### 27. `env_clear()` removes lockin's private tmp env and the scope does not restore it

Sections: SP3 step 7, lockin `SandboxedCommand::env_clear` docs.

Lockin's `SandboxedCommand::env_clear()` docs warn that it removes `TMPDIR`, `TMP`, and `TEMP` pointing at the per-sandbox private tmp. They are not re-injected automatically.

The scope mandates:

```rust
env_clear() first
```

and then injects only rafaello env, pass/set env, and no tmp env.

This may be acceptable, but it changes privacy behavior for programs that use temp dirs. They may write to `/tmp` instead of the sandbox-owned private tmp, subject to sandbox policy. The scope should explicitly decide whether to restore `TMPDIR`/`TMP`/`TEMP` to lockin's private tmp or intentionally clear them.

Right now it accidentally discards a lockin privacy default.

### 28. Lifecycle/Drop semantics are fantasy as written

Sections: SP1 shutdown, SP5, lifecycle tests.

The scope says:

- `Drop` for `SpawnedPlugin` sends SIGTERM.
- `PluginSupervisor::shutdown` sends SIGTERM, waits 5s, then SIGKILLs.
- Dropping supervisor kills children by end-of-test.

Problems:

1. Rust `Drop` cannot await a 5 second grace period.
2. `std::process::Child::kill()` sends SIGKILL on Unix, not SIGTERM.
3. lockin `SandboxedChild` exposes `kill()`, but not SIGTERM.
4. If a child exits and is not waited, it can become a zombie.
5. `kill -0` succeeds for zombies, so the proposed test "PID no longer alive" is unreliable.
6. Aborting the serve-loop join handle does not necessarily close all peer state in a deterministic order unless the transport is also dropped.

The scope needs a real lifecycle design:

- Use `nix::sys::signal::kill(Pid, SIGTERM)` if SIGTERM is required.
- Store child handles in a way they can be waited/reaped.
- Make `shutdown(self)` async if it waits.
- Define what `Drop` does synchronously: best-effort SIGKILL? spawn a reaper? abort tasks and drop handles only?

As written, lifecycle guarantees and tests are not implementable.

### 29. `PluginSupervisor::shutdown(self) -> Result<(), SpawnError>` cannot wait asynchronously as scoped

Section: SP1.

The scope says `spawn` is async, but writes the signature as:

```rust
PluginSupervisor::spawn(&mut self, plan: &CompiledPlugin) -> Result<&SpawnedPlugin, SpawnError> — async
```

For shutdown it writes:

```rust
PluginSupervisor::shutdown(self) -> Result<(), SpawnError>
```

but describes waiting up to 5 seconds. Waiting in Tokio should be async:

```rust
async fn shutdown(self) -> Result<(), SpawnError>
```

or the function must explicitly block the current thread. Blocking a Tokio runtime worker for process shutdown is not acceptable without saying so.

The public API signatures need to be written in valid Rust shape with `async fn` where intended.

### 30. `SpawnError` is a poor fit for shutdown errors

Sections: SP1/SP2.

`SpawnError` covers not-in-ACL, lockin build, spawn, proxy start, socketpair, fittings build, entry not executable. Shutdown can fail for different reasons:

- SIGTERM send failed.
- wait failed.
- timeout occurred and SIGKILL failed.
- join handle panicked/was cancelled.

Reusing `SpawnError` for shutdown muddies the error surface. Either define lifecycle variants or a separate `ShutdownError`.

### 31. `socketpair` flags and CLOEXEC are handwaved incorrectly

Section: SP3 step 2.

The scope says:

> CLOEXEC defaults are irrelevant here because lockin's `inherit_fd` mechanism deliberately survives the CLOEXEC sweep.

This is too glib. The parent-side fd must not leak to the child; the child-side fd must map to fd 3; and other fds must be sealed. `socketpair` should be created with explicit flags where possible:

- `SOCK_CLOEXEC` to avoid accidental leaks before lockin maps the intended fd.
- Maybe `SOCK_NONBLOCK` or set nonblocking later before Tokio conversion.

The scope should define exact `nix::sys::socket::socketpair` parameters and fd flag handling. Relying on "defaults irrelevant" is not acceptable in fd-authentication code.

### 32. Manual fd validation "exactly fds 0/1/2/3" is bogus

Section: Manual validation, Risk #6.

The manual validation says a running fixture should show exactly:

- fd 0
- fd 1
- fd 2
- fd 3

A Tokio Rust process can open epoll, eventfd, timerfd, signal, dynamic loader, locale, or other runtime fds. Depending on when `/proc/<pid>/fd` is observed, there may be more than four fds.

The right invariant is:

- fd 3 exists and is the bus socket;
- no duplicate inherited bus parent half leaked;
- no unexpected rafaello-controlled fds leaked.

Do not assert "exactly four fds" for a Tokio fixture.

### 33. Test harness observer subscription uses invalid pattern `**`

Section: integration harness.

The harness says it adds an observer fixture subscribed to:

```text
**
```

The canonical pattern grammar requires at least two segments. Current `validate_pattern("**")` returns `TopicTooFewSegments`.

Use explicit namespace patterns instead, for example:

- `core.**`
- `plugin.**`
- `provider.**`
- `frontend.**`

Or define a test-only observer with multiple grants. As written, the harness cannot compile a valid lock.

### 34. Integration tests require fixture behaviors not listed in F2

Sections: F2, positive/negative integration tests.

F2 lists supported fixture controls:

- publish hello
- publish bad namespace
- respond peer call echo
- call core ping
- open file
- publish bad grammar
- publish bad reply-to
- hold open

But the test matrix also requires fixture behavior for:

- `core.fixture.dump_env`
- `core.fixture.write_private_state`
- `core.fixture.report_open_result`
- side-channel ack from subscriber in `supervisor_bus_publish_round_trip.rs`
- possibly reporting write-denial errno/result

These are not specified in F2. The fixture protocol and test matrix are out of sync.

### 35. Private-state fixture lacks a way to know project root and topic id

Sections: SP6/F2/private-state test.

The private-state test says the fixture writes to:

```text
${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/marker
```

But supervisor injects only:

- `RFL_BUS_FD`
- `RFL_PLUGIN`

It does not inject project root or topic id. The fixture can derive neither reliably from `RFL_PLUGIN` unless it reimplements topic-id hashing and knows the project root. It can maybe use current working directory if supervisor sets it, but SP3 never sets `current_dir`.

The test needs one of:

- supervisor sets child `current_dir` to project root;
- supervisor injects `RFL_PROJECT_ROOT` and/or `RFL_TOPIC_ID` as reserved env vars;
- fixture receives target path through `plan.env.set`;
- test calls fixture with explicit path params.

As written, the fixture cannot know where to write.

### 36. Supervisor does not set child current working directory

Sections: SP3, private-state/read/write tests.

The scope never calls `SandboxedCommand::current_dir`. That means child cwd inherits from the test process unless lockin changes it. Tests and real plugins may assume project-root cwd, but this is not specified.

If rafaello plugins are supposed to run with project root as cwd, `CompiledPlugin` currently does not carry project root. If they are supposed to run with plugin dir as cwd, say so. If cwd is intentionally unspecified, tests must not rely on `${PROJECT_ROOT}` relative behavior.

This is a missing spawn contract.

### 37. Lockin denial tests are internally inconsistent

Sections: F2, negative tests.

F2 says for `RFL_FIXTURE_OPEN_FILE`:

> The fixture does not assert; the test reads the fixture's exit code (or watches stderr)

The negative read-denial test says:

> Fixture has `RFL_FIXTURE_OPEN_FILE=/etc/passwd` and `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.report_open_result`. Test calls `report_open_result`; fixture replies with errno.

Those are different fixture designs. Pick one:

- immediate attempt and process exit/status assertion;
- persistent service that stores result and reports over peer.call.

The latter is better for deterministic tests, but then F2 must specify it.

### 38. `supervisor_drop_during_spawn_unwinds` proposes a non-failure

Section: negative tests.

The test says force failure at SP3 step 8 using:

> `entry_absolute` points at `/usr/bin/false` so the child exits immediately

That is not a spawn failure. `command.spawn()` succeeds. The child exits after spawn.

If the supervisor treats early child exit as spawn failure, the scope must define a handshake/readiness window. Otherwise this test does not exercise step-8 failure.

Use a real spawn failure condition if that is what the test needs, such as an executable path that disappears before spawn, permission denied, or invalid interpreter. But note `EntryNotExecutable` catches some cases earlier.

### 39. `EntryNotExecutable` may be unreachable for fixture binaries under lockin if exec grants are wrong

Sections: SP3 step 4/6, F1/W3.

The fixture binary path is in target dir. The sandbox builder always allows the `program` itself to be executed internally in lockin, but the scope also loops over `plan.filesystem.exec_paths/exec_dirs` from m1. It does not state whether the entry binary must appear in `exec_paths` or is implicitly executable by lockin. Lockin's Linux backend appears to add the program path to exec allow rules automatically.

The scope should explicitly state that `plan.entry_absolute` is always executable as the program regardless of `FilesystemPlan.exec_*`, and `exec_*` grants are for subprocesses the plugin launches. Otherwise implementers/tests may grant the fixture binary unnecessarily or misunderstand lockin behavior.

### 40. The scope does not define stdio policy for spawned plugins

Sections: SP3, SP5, manual validation.

SP3 never sets stdin/stdout/stderr. Manual validation assumes stdio is inherited from cargo test. The fixture emits `eprintln!` for hard failures. Tests may need stderr capture.

Define whether supervisor:

- inherits stdio,
- nulls stdin,
- pipes stdout/stderr,
- captures logs for test inspection.

This matters for fd leak assertions and deterministic tests.

### 41. `BusEvent` and `PublishMsg` omit `request_id` despite overview envelope

Sections: B1/B7, overview §4.5.

Overview §4.5 says every event carries `request_id` when applicable. `PublishMsg` includes only:

- topic
- payload
- in_reply_to
- taint

No `request_id`.

Maybe m2 intentionally omits event-level request ids because tool dispatch is deferred. If so, the scope needs an explicit reconciliation note. Otherwise this looks like spec drift from the canonical bus event envelope.

This is especially relevant because `in_reply_to` is an array of prior request ids, while `request_id` is the id of the current event/request. They are not interchangeable.

### 42. Taint handling contradicts overview text unless explicitly staged

Sections: B1, overview §4.5, security RFC §7.2.

B1 says m2 stores plugin-supplied taint verbatim, with no synthesis and no superset check.

Overview says plugin authors never write taint directly, though they may add to it and core verifies the superset rule.

The scope says this is m4 work, which may be acceptable. But then m2 must not let any m2-delivered event be interpreted as security-validated taint. Tests and docs should distinguish "taint field round-tripped but untrusted" from "taint envelope enforced".

Currently the scope's wording could mislead implementers into preserving plugin-supplied taint as if it were authoritative.

### 43. `in_reply_to` arity errors are underspecified

Sections: B2/B6/negative tests.

B6 says `tool_result` and `rpc_reply` require exactly one entry. B2 only has:

```rust
MissingInReplyTo
```

What error is returned when `in_reply_to` is present but empty or has two entries? It is not missing. It is wrong arity.

The negative tests cover missing only. Add either:

- `InvalidInReplyTo { canonical, topic, reason }`, or
- broaden `MissingInReplyTo` to include arity and rename it.

Also add tests for `[]` and `[id1, id2]`.

### 44. `TaintEntry` should probably deny unknown fields too

Sections: B1/B4.

B4 says `PublishMsg` has `#[serde(deny_unknown_fields)]`, so extra keys at the top level are rejected. It does not say `TaintEntry` denies unknown fields.

If `TaintEntry` is also wire schema, unknown fields inside taint entries should be rejected or explicitly allowed. Otherwise a plugin can smuggle arbitrary metadata under taint entries while top-level strictness gives a false sense of schema enforcement.

### 45. `Broker::publish_core` has no lifecycle/boot summary definition

Sections: B1, Goal.

B1 says `publish_core` is used by m2 only in tests plus a single `BootSummary` event. The scope never defines:

- topic name,
- payload schema,
- when it fires,
- what subscribes to it,
- test coverage for it.

Either define the BootSummary event or remove it from m2. A vague extra event is scope creep.

### 46. `Broker` clone/Arc shape is underspecified for mutation and locking

Sections: B1/B7/SP3.

Broker owns live registry and ACL and is cheap-to-clone. Fan-out iterates registered plugins while drops/unregisters may happen concurrently. The scope does not define:

- lock type (`Mutex`, `RwLock`, `DashMap`, etc.);
- whether fan-out snapshots peers before sending;
- whether `RegisteredPlugin::drop` can deadlock while fan-out logs/sends;
- whether fan-out order is deterministic for tests.

This can be left to implementation if tests avoid order assumptions, but the scope's test matrix includes multi-plugin fan-out. It should at least require snapshot-before-send and no lock held across `peer.notify` to avoid deadlocks/reentrancy.

### 47. Fan-out error handling is too vague for `PeerHandle::notify`

Sections: B7, fittings current API.

`PeerHandle::notify` returns `Ok(())` even when the bounded sink is full; it logs internally and records dropped notifications. It returns `Err` when the notification channel is closed.

B7 says fan-out errors are logged and do not fail publish. That is fine, but tests asserting delivery under load need to understand that full queues are not errors. If m2 does not configure small capacities, fine. But the scope should use actual fittings semantics: `Ok` does not mean delivered.

### 48. The scope does not define what happens to inbound notification errors

Sections: B2/B4/SP3 step 11/F2 bad publish behavior.

F2 says the fixture attempts a bad publish and does not exit on error because broker silently rejects notifications since they have no response.

But the core-side `Service::call` for a notification still returns a `Result<Response, FittingsError>` to fittings; fittings drops responses for notifications. If the handler returns `Err`, does fittings log? Does it close? Does it ignore? The scope should define the service behavior:

- For invalid `bus.publish`, broker records/emits rejection and service returns `Ok(dummy_response)` so transport remains alive.
- Or service returns a JSON-RPC error that is dropped because notification, but does not close.

Do not leave notification error behavior accidental.

### 49. Unknown `bus.publish` methods and request/notification distinction are not scoped

Sections: SP3 step 11/F3.

The per-plugin service handles inbound `bus.publish` notifications. What if plugin sends `bus.publish` as a request with an id? What if it sends unknown methods? What if it sends `bus.event` back to core? Fittings will invoke `Service::call` for both requests and notifications, with `Request.id = None` for notifications.

The bus service should explicitly:

- accept `bus.publish` only as notification or also request?
- reject `bus.publish` requests with a structured error?
- reject unknown methods with MethodNotFound?
- reject plugin-originated `bus.event`?

This is part of the bus protocol surface.

### 50. `BrokerAcl` and `CompiledPlugin` publish/subscribe fields can diverge

Sections: Inputs, B1, SP3.

m1 emits both:

- `BrokerAcl` containing plugin ACL;
- `CompiledPlugin` containing `subscribe_patterns`, `publish_topics`, `auto_subscribes` fields, but current compile code returns these as empty vectors.

m2 broker uses `BrokerAcl`, while supervisor takes `CompiledPlugin`. The scope should explicitly say the supervisor ignores the ACL fields on `CompiledPlugin` and uses only `BrokerAcl` for authority. Otherwise the duplicate fields are confusing and may diverge.

Current `compile_plugin` in the repo sets those vectors to `Vec::new()`, so any implementation relying on `plan.publish_topics` would be wrong.

### 51. The scope claims m2 does not modify m1 surfaces, but B8 and error additions do

Sections: Inputs, B8, E.

Inputs says m2 calls into m1 and does not modify them. But B8 modifies validation API and E modifies `error` top-level enum and `lib.rs` re-exports.

Error additions are expected, but the statement "m2 calls into these; it does not modify them" is overbroad. Either narrow it or acknowledge specific m1-surface additions.

### 52. Spawn failure unwinding is under-specified for each resource

Sections: SP3 unwinding contract, negative tests.

SP3 says failure before broker registration tears down resources allocated earlier. But each failure point has different resources:

- socketpair before proxy;
- proxy handle before builder;
- moved child fd inside builder/command;
- spawned child before fittings build;
- server built but not registered;
- registered broker guard before serve loop spawn;
- serve loop spawned but later storage fails.

The scope needs a table or precise contract for each failure phase. Otherwise tests like fd-count before/after will be flaky and implementers will miss cases.

### 53. Duplicate supervisor spawn is not tested

Sections: SP3, negative tests.

`Broker::register_plugin` can return `AlreadyRegistered`, but there is no supervisor integration test for spawning the same canonical twice. That case matters because the second spawn may already have allocated socketpair/proxy/child before registration fails at step 12.

Add a negative test:

- Spawn plugin A successfully.
- Attempt to spawn A again.
- Expect duplicate/AlreadyRegistered mapping.
- Assert second child/proxy/fd are cleaned up.

### 54. `PluginSupervisor` internal registry shape prevents RAII clarity

Sections: SP1/SP5.

SP1 says supervisor holds `Vec<SpawnedPlugin>`. SP5 says dropping `SpawnedPlugin` kills the child and unregisters broker. `PluginSupervisor::shutdown(self)` consumes supervisor.

But if `spawn` returns `&SpawnedPlugin`, callers cannot drop individual plugins independently. If `Drop` on `SpawnedPlugin` kills children, dropping the supervisor's Vec kills all children, but borrowed references become invalid. The public lifecycle model needs to be coherent:

- Is `SpawnedPlugin` owned only by supervisor?
- Can a caller stop one plugin by dropping a handle?
- Does dropping a clone kill the plugin or only last handle?

Current text mixes internal storage and public RAII in an unclear way.

### 55. `PeerHandle` lifetime across plugin shutdown is underspecified

Sections: SP5, Out of scope PeerHandle call correlation.

Out of scope says lifecycle drops `PeerHandle` on plugin exit and in-flight calls resolve with `FittingsError::Transport` per fittings contract.

But SP5 only aborts the serve-loop join handle and drops registration guard. It does not define a child-exit watcher that notices plugin exit and closes the transport/peer. If the child exits but the core still holds fd/serve task, closure may be detected eventually by read EOF, but tests need deterministic behavior.

Define whether m2 has a reaper task that monitors child exit and unregisters the plugin, or whether registration persists until supervisor/drop.

### 56. Plugin exit does not automatically unregister in the scoped design

Sections: SP5/B1.

`RegisteredPlugin` guard is held inside `SpawnedPlugin`. If the child exits but `SpawnedPlugin` remains in supervisor registry, the guard remains alive and broker still considers the plugin registered. Fan-out will attempt to notify a dead peer until the serve loop detects closure.

If m2 wants live registry to reflect process liveness, it needs a task that drops the registration on transport closed/child exit. If not, tests should not assume child exit unregisters immediately.

The scope currently says RAII drop unregisters, but says little about natural child exit.

### 57. `No hardcoded bypass` does not address unsandboxed execution through granted exec paths

Sections: SP4/security RFC §6.9.

SP4 says no `spawn_unsandboxed`. Good. But a plugin granted broad `exec_paths` can execute subprocesses inside the same sandbox. Security RFC §6.9 accepts partial risk. m2 should ensure the supervisor does not accidentally expose lockin builder knobs like `allow_non_pie_exec`, `allow_interactive_tty`, or raw rules. It says not to call those knobs, which is good.

However, tests only prove file denial for the fixture itself, not child processes launched by the fixture. If m2 claims partial closure of 6.9, add a test or weaken the claim. The Inputs section says m2 closes partial 6.9, but the matrix does not prove exec subprocess confinement.

### 58. `RFL_BUS_FD_NUMBER = 3` needs collision/error handling

Sections: SP3 step 5.

The scope fixes fd 3. Good for convention. But what if the parent process already has important fd 3 open? `inherit_fd_as` maps in the child, but the parent command setup must ensure target fd collision in child is handled as intended. lockin's `map_fd` likely handles it, but the scope should cite/assume that mapping fd 3 overwrites/closes any previous child fd 3 after the fd seal.

Tests should not assume parent fd numbers.

### 59. The fixture protocol assumes bus fd parsing from env but not fd ownership safety

Sections: F3.

F3 says parse `RFL_BUS_FD` as `u32`, wrap in `UnixStream::from_std` after `set_nonblocking(true)`.

Rust raw fd conversion must use `FromRawFd` exactly once. The fixture must not leave duplicate `OwnedFd`/`File` wrappers around fd 3. The scope should explicitly require safe ownership transfer from raw fd to `OwnedFd`/`UnixStream` once.

This is easy to get wrong and can cause double-close.

### 60. Tests mutate parent environment without adding the dependency/strategy

Sections: supervisor_env_pass_set_applied test, Risk #4.

The test mentions `temp_env::with_var` or equivalent. W1 does not add `temp-env` or `serial_test`. Risk #4 punts strategy to `commits.md`.

This is too late for a test matrix that must be executable. Add the chosen dev dependency and serialization strategy to W1/I.

### 61. Linux-only test gating is incomplete

Sections: Out of scope macOS, Demo bar.

The scope says spawn-bearing tests are Linux-only and macOS ignored. But lockin/syd availability is not only OS-dependent. Linux machines outside the devshell may not have syd. Risk #5 says outside-devshell `cargo test` is unsupported, but Acceptance summary still says:

```text
cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core green on Linux
```

That is too broad. Either tests must detect missing syd and skip with a clear message, or acceptance must say Linux inside nix devshell. Manual validation includes devshell but the main acceptance bullet does not.

### 62. `cargo test -p rafaello-core` conflicts with fixture workspace member strategy

Sections: W3, Acceptance.

Acceptance says:

```text
cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core
```

and separately:

```text
cargo build -p rfl-bus-fixture
```

But if `rafaello-core` tests need the fixture binary path at compile time via `env!`, a separate later `cargo build -p rfl-bus-fixture` does not help. The test build itself must have the env var.

This is the same issue as finding #5, but it also invalidates the acceptance commands.

### 63. The fixture crate being "built unconditionally by the workspace" is false

Section: W3.

Cargo workspaces do not build every member unconditionally when testing one package with `-p rafaello-core`. They build selected packages and dependencies. A standalone fixture binary crate that is not a dependency of `rafaello-core` will not be built.

If the fixture is added as a dev-dependency, Cargo builds its library target, not necessarily its binary target. A binary-only crate as a dev-dependency is also not a normal pattern.

The scope must not rely on unconditional workspace builds.

### 64. `assert_cmd` is added but not actually used by the stated path strategy

Section: W1/W3.

W1 adds `assert_cmd = "2"`, but W3 uses `env!("CARGO_BIN_EXE_rfl-bus-fixture")`. `assert_cmd` is not needed for that. If the intended strategy is `assert_cmd::cargo::cargo_bin`, the scope should say so and verify cross-package behavior.

Right now the dependency looks cargo-culted.

### 65. The broker tests require fake `PeerHandle`s but no testkit strategy is scoped

Sections: broker_publish_core, broker_register_plugin, B1.

Broker unit/integration tests need a `PeerHandle` to observe `peer.notify("bus.event", ...)`. Current `PeerHandle` constructors are in `fittings_core::context` and require internal channel types that may be public, but this is not a clean test API. Alternatively tests can run real fittings clients.

The scope does not say how broker-only tests construct observable peers. If using real fittings over in-memory transport, add dependency/testkit. If using `PeerHandle::new`, verify the needed types are public and stable.

### 66. The scope says `BrokerError` lives in `rafaello_core::error`, but B1's serde decode errors need serde_json context

Sections: B2/B4/E.

If `InvalidPayload` carries only `reason: String`, tests may be brittle or under-specified. If decode happens in Service layer, the error may be `serde_json::Error` not a `BrokerError`. The scope should define exact mapping from serde errors to `InvalidPayload` and lifecycle rejection reason.

### 67. Top-level `Error` enum conversion from `SpawnError` may be awkward if `SpawnError` contains non-`Send + Sync` sources

Sections: E/SP2.

If `Lockin { source: anyhow::Error }` is used, derive `thiserror` works, but top-level error traits and source bounds should be checked. If `SpawnError` includes `JoinError` or boxed errors later, define the shape carefully.

This is not hard, but current SP2 references nonexistent types and E assumes smooth `#[from]` arms.

### 68. The scope names `validate::pattern::matches` but current module layout has no `validate/pattern.rs`

Sections: B8/manual validation.

Manual validation says module additions include `validate/pattern.rs` extension. Current validation module has `validate/topic.rs`. Creating a new `pattern.rs` is possible but unnecessary and would require updating `validate/mod.rs`. If the scope wants the new module, it should justify the move and include compatibility with existing re-export.

### 69. `publish_topics ∪ auto_subscribes` also violates m1 `BrokerAcl` meaning

Sections: B3, broker_acl.rs.

`BrokerAcl::PluginAcl` separates:

- `publish_topics`
- `subscribe_patterns`
- `auto_subscribes`

The compiler inserted `auto_subscribes` specifically as a subscription to `plugin.<topic-id>.tool_request`. Treating it as publish authority conflates directions and undermines the ACL type.

This reinforces finding #9.

### 70. The scope does not define subscribe pattern validation at broker construction

Sections: B1/B7.

m1 `broker_acl::compile` validates patterns. But B5 says runtime revalidation is symmetric for absent runtime subscribe path. Broker construction accepts a `BrokerAcl` directly, and tests may hand-construct invalid ACLs.

Should `Broker::new` trust `BrokerAcl`, or validate it defensively? The scope only requires publish topic revalidation. If invalid subscribe patterns are present in hand-constructed ACLs, `pattern_matches_topic` assumes valid inputs but returns false for malformed patterns. Is that acceptable? State it.

### 71. Core publish authority event rejection is not tested for invalid core topics

Sections: broker_publish_core positive test.

There is a negative test named `broker_publish_core_namespace_rejected.rs`, but the table description says "Plugin publishes `core.session.user_message`". That is not `publish_core` rejecting a namespace; it is plugin publish reserved namespace rejection.

Missing tests:

- `Broker::publish_core("plugin.x.y", ...)` rejects.
- `Broker::publish_core("provider.x.y", ...)` rejects.
- `Broker::publish_core("core.Bad", ...)` rejects grammar.

The test file name is misleading.

### 72. `frontend.*` handling in m2 has no frontend principal path, but broker error conflates it with plugin error

Sections: B2/B3/out of scope frontends.

For m2, only plugin connections exist. `PublishOnReservedNamespace` covers plugin publishing `frontend.*`. Later, frontend principals will be allowed to publish `frontend.<attach-id>.*`. If `BrokerError::PublishOnReservedNamespace` hardcodes canonical plugin id, future extension may be awkward.

This is not necessarily blocking for m2, but the scope should say the broker currently has only plugin registrations and frontend principal support is not represented in the API.

### 73. `Broker::register_plugin(canonical, peer)` does not validate peer liveness

Sections: B1/SP3.

Registering a peer before serving starts is intended. But if transport build or serve-loop spawn fails after registration, guard cleanup must happen. If peer is already closed, register succeeds and fan-out later fails. That may be acceptable, but tests should not assume register implies live peer.

Scope should define register as registry-only, not handshake.

### 74. `SpawnedPlugin` public fields include `peer`, but no notification subscription for tests

Sections: SP1/F3/tests.

`peer` lets tests call fixture methods. It does not let tests observe bus events unless the fixture implements a service/ack path. The harness says it returns a one-shot receiver for inbound `bus.event` deliveries from an observer plugin, but does not specify how the observer forwards those events to the harness.

Need a concrete observer mechanism:

- fixture notification handler calls back to core via `peer.call("core.fixture.observed", event)`;
- fixture writes to a side-channel fd;
- test subscribes to client notifications if test itself is a fake plugin.

Currently observer implementation is under-specified.

### 75. The fixture's `RFL_FIXTURE_PUBLISH_BAD_NAMESPACE` cannot observe broker rejection

Sections: F2/negative tests.

F2 says bad namespace publish does not exit on error and assertion is broker-side. But the negative tests are named `broker_publish_*`, not `supervisor_*`, and some likely call broker directly. If testing through fixture, how does the test observe `BrokerError::PublishOnReservedNamespace` when notifications have no responses?

Options:

- Direct broker unit tests call `handle_plugin_publish` and assert error.
- Supervisor integration tests observe `core.lifecycle.publish_rejected` event.

The scope mixes these. Each negative should say direct broker API or fixture integration.

### 76. `core.lifecycle.publish_rejected` no-fanout assertion conflicts with rejection event fanout

Section: negative tests.

Namespace rejection tests say:

> a `core.lifecycle.publish_rejected` event fires; no fan-out.

No fan-out of what? The rejected original event should not fan out. The rejection event itself must fan out to subscribers. The tests should phrase this precisely:

- original forbidden topic is not delivered;
- rejection lifecycle topic is delivered to subscribers of `core.lifecycle.**`.

### 77. The test matrix names many separate files but acceptance allows merge/split

Sections: Demo bar/Acceptance.

This is fine, but `commits.md` must preserve behavior coverage. The scope already says tests may split/merge. Keep that.

### 78. The scope claims m2 closes attack 6.8 via env clearing, but dynamic linker env is already stripped by lockin

Sections: Inputs/Risks/SP3 step 7.

Security RFC attack 6.8 is LD_PRELOAD. Lockin `SandboxedCommand::env` silently drops dynamic linker blocklist keys and strips them again before spawn. m2 also calls `env_clear`. This is fine, but if claiming m2 closes 6.8, tests should include env-set/pass dynamic linker attempts or rely on lockin's existing tests. The current matrix only covers reserved RFL env, not LD_PRELOAD.

### 79. `allow_non_pie_exec` default may break fixture binaries on Nix

Sections: SP3 step 4, Risks.

The scope intentionally does not call `allow_non_pie_exec`. Lockin docs say syd denies non-PIE by default and this can break toolchains whose compilers are built without PIE, notably on Nix. The fixture binary itself is likely PIE, but if tests execute `/usr/bin/false` or other system binaries, this could matter.

At minimum, avoid using `/usr/bin/false` as a test fixture for spawn-failure. Keep fixture execution under controlled build outputs.

### 80. `max_cpu_time` and `max_open_files` are not optional in current `LimitsPlan`

Sections: SP3 step 4, m1 compile.

SP3 says:

> `max_cpu_time`, `max_open_files`, `max_address_space` (only if Some), `max_processes` (only if Some)

Current `LimitsPlan` has:

```rust
pub max_cpu_time: u64,
pub max_open_files: u64,
pub max_address_space: Option<u64>,
pub max_processes: Option<u64>,
```

So `max_cpu_time` and `max_open_files` are always present after compile defaults. The scope wording is mostly fine but should be precise: call those unconditionally; call the optional ones only if `Some`.

### 81. `disable_core_dumps()` "on lockin Linux" is odd because builder API is cross-platform

Section: SP3 step 4.

The scope says disable core dumps unconditionally on lockin Linux. The builder method exists cross-platform and configures rlimits. If intended only on Linux, say why. If safe everywhere, call it unconditionally.

### 82. The scope does not include `current_dir`/args handling for plugin entry

Sections: SP3.

Manifest entry is a path, not an argv array. If plugin needs args, not in v1. Fine. But current dir remains unspecified (finding #36). This matters more than args.

### 83. Tests that mutate `CompiledPlugin` by hand conflict with claim that constructor is private

Sections: Risks #7, negative tests.

Risks #7 says:

> the only constructor is via `compile::compile_plugin`; the `pub` fields exist for read access only and the compiler doesn't expose a mutation API.

But Rust public fields mean any crate can construct and mutate the struct directly. The negative tests explicitly hand-construct/mutate `CompiledPlugin`. The statement "only constructor" is false.

If m2 relies on `CompiledPlugin` integrity, make fields private or stop claiming construction is controlled.

### 84. `BrokerAcl` is also publicly constructible

Sections: B1/SP3.1.

`BrokerAcl` and `PluginAcl` have public fields. A caller can create an ACL that was not compiled from a lock. That is fine for tests, but then the supervisor's "lock entry" guarantee is not enforceable by taking `BrokerAcl` alone. The scope should treat `BrokerAcl` as authority input trusted by core, not as proof of a lock.

### 85. `SpawnError::NotInAcl` should preserve source from broker registration failure

Sections: SP2/SP3 step 12.

If broker registration fails because canonical not in ACL or already registered, mapping to `SpawnError` should preserve enough information for tests. Current SP2 only lists `NotInAcl`. Add duplicate mapping.

### 86. No m2 API for reading broker ACL from supervisor

Sections: SP3.

SP3 step 1 says verify plan canonical is in `broker.acl.plugins`, but B1 does not expose broker ACL or a `contains_plugin` helper. If `Broker` keeps `BrokerAcl` private inside `BrokerInner`, supervisor needs an API:

```rust
Broker::plugin_acl(&CanonicalId) -> Option<PluginAcl>
```

or:

```rust
Broker::contains_plugin(&CanonicalId) -> bool
```

Otherwise supervisor reaches into internals.

### 87. `Broker::new(acl: BrokerAcl)` vs `PluginSupervisor::new(broker: Arc<Broker>)` Arc shape is inconsistent

Sections: B1/SP1.

B1 says `Broker` itself is cheap-to-clone `Arc<BrokerInner>`. SP1 takes `Arc<Broker>`. That is double-Arc unless `Broker` is not actually internally Arc.

Choose one:

- `Broker` is a cloneable handle; supervisor stores `Broker`.
- `Broker` is a concrete struct; supervisor stores `Arc<Broker>`.

Current text says both.

### 88. `Broker::register_plugin` takes `PeerHandle` by value; tests may need clone semantics defined

Section: B1/SP1.

`PeerHandle` is cloneable. Broker can store a clone, but if it takes by value, caller may lose its own handle unless it clones before calling. SP3 says `SpawnedPlugin` stores `peer` and broker stores registration. The sequence should say clone explicitly:

```rust
let peer = server.peer();
let guard = broker.register_plugin(canonical.clone(), peer.clone())?;
```

### 89. `BrokerError::Internal` for channel send is misleading under fittings semantics

Section: B2/B7.

`PeerHandle::notify` full queue returns `Ok(())`, closed channel returns `FittingsError::Transport`. B7 says fan-out errors do not fail publish. Therefore channel send errors should not become `BrokerError::Internal` in normal fan-out. `Internal` may still exist for lock poisoning/serialization, but define what actually maps to it.

### 90. Missing rejection for invalid top-level `provider`/`frontend` short topics

Sections: B3.

`provider.x` and `frontend.x` are grammar-valid two-segment topics. B3 catches `topic starts with "provider."` and `frontend.`, so those are rejected for plugins. Good. But unknown top-level and short plugin topics remain holes.

### 91. Pattern matcher semantics for `**` one-or-more need tests for zero trailing segments

Sections: B8.

The existing `pattern_matches_topic` implements `**` as one or more trailing segments. Tests should include:

- `core.session.**` does not match `core.session`.
- `plugin.id_x.**` does not match `plugin.id_x`.

The scope mentions the example matrix but not the zero-trailing negative. Add it if not already in existing tests.

### 92. `JsonRpcId` import source is wrong/ambiguous

Sections: B1/B4.

The scope says `JsonRpcId` per fittings-wire's type. Current overview says `fittings-core` becomes canonical owner and fittings-wire re-exports. Current code has `fittings_core::message::JsonRpcId` and `fittings_wire::types::JsonRpcId` imported in server. The scope should use the canonical current type path, likely `fittings_core::message::JsonRpcId`, unless there is a deliberate wire re-export.

### 93. `PublishMsg` `in_reply_to: Option<Vec<JsonRpcId>>` with `null` ids needs explicit serde behavior

Section: B4/B6.

`JsonRpcId` includes `Null`. That means `in_reply_to: [null]` is valid by type. Is that acceptable? The fittings RFC preserves null ids. For m2 arity checks, yes. Later correlation maps may need to handle `Null`. Scope should not accidentally reject null if fittings supports it.

### 94. `deny_unknown_fields` on `PublishMsg` will reject future extensions

Section: B4.

This is probably intentional for security. But if m3/m4 add fields, they must update the struct. Fine. Keep it.

### 95. No schema for `payload` object vs any JSON value

Section: B4.

B4 allows any JSON value including null. Security RFC bus event payloads are kind-specific JSON objects. For generic m2 broker tests, any JSON may be okay. But if `core.lifecycle.publish_rejected` payload is object schema, enforce object there.

### 96. `RFL_FIXTURE_PUBLISH_BAD_GRAMMAR=plugin.id_xxx.UPPERCASE` may not be outside grant

Section: F2/negative tests.

If the broker checks grammar before ACL, this returns `InvalidTopic`. If it checks namespace/ACL before grammar, it might return `PublishOnReservedNamespace` or `PublishOutsideGrant`. The scope says validate authority then grammar in B1 but B5 says every publish reruns grammar. Define ordering for deterministic errors.

Security-wise, grammar first is cleaner: parse only valid topics. But publisher identity checks may need topic id. Pick one and align tests.

### 97. Error precedence is underspecified

Sections: B3/B5/B6.

For a topic like `core.Bad`, should broker return reserved namespace or invalid grammar? For `plugin.<other>.UPPER`, reserved namespace or invalid topic? Tests may become brittle unless precedence is specified.

Recommended order:

1. Validate topic grammar.
2. Parse namespace structurally.
3. Enforce namespace/ACL.
4. Enforce payload/in_reply_to class rules.

Or if rejecting reserved namespace even for malformed topics is desired, say so.

### 98. `PublishOutsideGrant` exact-match semantics may be too narrow for grant patterns

Section: B3.

m1 grant `publishes` appears to be topic literals, not patterns. Good. The scope should explicitly state publish grants are exact topic literals, not patterns, and membership is exact string equality after validation.

### 99. Subscribe fan-out should precompute pattern lists or validate on registration

Section: B7.

Fan-out checks every registered plugin's `subscribe_patterns ∪ auto_subscribes` on every publish. Fine for m2. But if invalid pattern sneaks into hand-built ACL, matcher assumes validation. Either validate at Broker::new/register or tolerate false.

### 100. `PluginSupervisor::spawn` signature says async in prose but not in code

Section: SP1.

The public surface bullet should write:

```rust
pub async fn spawn(&mut self, plan: &CompiledPlugin) -> Result<..., SpawnError>
```

and:

```rust
pub async fn shutdown(self) -> Result<(), ...>
```

if async. The current prose "— async" after a non-async signature is imprecise.

### 101. `SpawnedPlugin` cannot be `Debug`/opaque if it stores guard/proxy/child but tests need fields

Section: SP1.

The scope says `SpawnedPlugin { canonical, topic_id, child_pid, peer, ... }` opaque-ish. Fine, but make sure public fields do not force exposing non-clone child/proxy types. A handle/view type may be cleaner.

### 102. `PluginSupervisor::shutdown(self)` consuming self makes failure recovery impossible

Section: SP1.

If shutdown returns `Err`, supervisor is consumed. Maybe fine. But if some children failed to die, caller has lost handles. Consider returning a report rather than a single error, or best-effort shutdown that always consumes and reports failures.

### 103. Drop order matters and is not specified

Section: SP5.

SP5 lists sending SIGTERM, dropping proxy, dropping registration, aborting serve loop. The order should be deliberate:

- Stop accepting/sending bus first?
- Unregister before killing child to stop fan-out?
- Kill child before dropping proxy to avoid plugin making final network calls during grace?

Security-wise, unregister and close bus promptly, then terminate child, then drop proxy after child is dead or immediately to cut network. The current prose is not enough.

### 104. Proxy handle drop before child kill can alter shutdown behavior

Section: SP5.

If proxy handle drops before the child exits, any in-flight plugin network calls fail. That may be desirable during shutdown. But if graceful shutdown expects plugin to flush network telemetry, it prevents it. m2 likely wants fail-closed. State the order.

### 105. No handling for child process groups

Sections: SP5/security RFC §6.9.

If a plugin launches subprocesses, killing only the direct child PID may leave descendants unless lockin/syd process supervision handles them. lockin `SandboxedChild::kill()` kills the child, not necessarily the process tree. The scope claims supervisor owns child processes, but not process trees.

If lockin provides process-tree containment, cite it. Otherwise the shutdown guarantee is too strong.

### 106. `max_processes` may prevent normal runtime threads/processes

Section: SP3 step 4.

m1 defaults `max_processes` to `None`; okay. If tests set it, Tokio fixture may create threads but not processes. Fine. Not a blocker.

### 107. `outpost_proxy::start` has no call-count observability

Section: `supervisor_proxy_starts_for_proxy_plan.rs`.

The test says:

> `outpost_proxy::start` is called once

Without dependency injection or tracing hooks, the test cannot assert call count. It can assert a proxy handle/listen port exists if supervisor exposes it, but `SpawnedPlugin` public fields do not include proxy port. The scope must either expose test-only proxy info, inject a proxy starter trait, or weaken the assertion.

### 108. Lockin policy "gets `network_proxy(port)`" is not directly observable

Section: proxy positive test.

The test says assert lockin policy gets `network_proxy(port)`. The supervisor calls builder methods; there is no public inspection after builder consumption unless using a fake builder. Integration test can only observe behavior, not builder call. Since the fixture does not issue CONNECT, the current test cannot prove lockin received proxy mode.

Add a fixture behavior that attempts direct non-proxy TCP and/or proxy env CONNECT, or introduce a builder abstraction for unit tests.

### 109. Filesystem mapping tests cannot inspect builder calls either

Sections: SP3 step 4, lockin denial tests.

Unlike proxy, filesystem denial tests do observe behavior (`/etc/passwd` denied, private state writable). Good. The scope should lean on behavioral tests rather than builder-call assertions.

### 110. `supervisor_lockin_denies_outside_grant_write` target may be inside read grant but outside write grant

Section: negative write test.

The test attempts to write `${PROJECT_ROOT}/forbidden`. If project root is readable but not writable except private state, good. Ensure the lock grants read to project root if needed but write only to private state. Also ensure the parent test can check nonexistence after child attempt.

### 111. `/etc/passwd` denial may be invalid if dynamic loader/system reads need `/etc`

Section: negative read test.

Lockin's Linux backend automatically allows some system reads (`/etc/ld.so.cache`, `/etc/ld.so.preload`) but not `/etc/passwd`, so test is likely okay. But on systems with NSS/user lookup, runtime may read `/etc/passwd` before fixture behavior. The fixture explicitly reading `/etc/passwd` should fail under sandbox. Keep Linux-only.

### 112. Missing test for entry path not found vs not executable

Section: SP2/negative tests.

m1 compile catches entry not found, but hand-mutated plans can point to missing path. SP2 only has `EntryNotExecutable`, not `EntryNotFound`. `SandboxBuilder::command` may succeed and spawn fails with io error. Decide whether supervisor prechecks existence and file type or only exec bit.

### 113. `EntryNotExecutable` before builder consumption may conflict with symlink executable targets

Section: SP3 step 6.

If `entry_absolute` is a symlink to executable file, `metadata` vs `symlink_metadata` changes result. Define symlink behavior.

### 114. Fixture crate may need its own dependency list beyond fittings-core

Sections: F3/W3.

F3 says fixture uses fittings-core. To actually connect it needs fittings-client and fittings-transport or a custom transport, plus tokio, serde_json, async-trait if service. W3 should specify fixture crate dependencies.

### 115. Fixture should probably not depend on `rafaello-core`

Section: F.

If fixture imports `TaintEntry`/`PublishMsg` from `rafaello-core`, that can create circular-ish test coupling and expose internal APIs. It can either use `serde_json::json!` wire shapes or depend on public types deliberately. Scope should choose.

### 116. `RFL_FIXTURE_HOLD_OPEN` and signal handling are underspecified

Section: F2/SP5.

If the fixture sleeps until SIGTERM, how does it handle SIGTERM? Default action terminates. Good. But if Tokio runtime ignores? Default Unix signal terminates process unless handler installed. Fine. For graceful shutdown tests, fixture need not handle.

### 117. `core.fixture.ping` service is not part of supervisor surface

Section: `supervisor_peer_call_plugin_to_core.rs`.

SP3 step 11 says the per-plugin Service handles inbound `bus.publish` notifications. The test requires it also respond to `core.fixture.ping` by echoing `+1`. This is test-only behavior not in public supervisor API.

Need a way for tests to inject extra per-connection Service methods into supervisor. Otherwise production supervisor's service will return MethodNotFound and the fixture exits 2.

Options:

- `PluginSupervisor::new_with_service_factory(...)` test-only/public hook.
- Harness bypasses supervisor service? Not possible if using real path.
- Fixture calls `bus.publish` instead of `peer.call` for plugin-to-core test.

As written, the test cannot be implemented with the public surface.

### 118. `core.fixture.echo` core-to-plugin call is fine, but method namespace is odd

Section: supervisor_peer_call_core_to_plugin.

Core calling plugin method `core.fixture.echo` is a method name, not a bus topic. It does not violate publish namespace, but the prefix may confuse readers. Consider `fixture.echo` unless the method-name namespace is intentionally independent.

### 119. The fixture's `bus.publish` notification result cannot be observed by the fixture

Section: F2.

Correct: notifications have no response. The scope understands this. Tests must observe broker side. Keep that distinction clear.

### 120. `Broker::handle_plugin_publish` returning `Result` is not enough for notification path

Sections: B1/SP3.

For direct broker tests, `Result` is good. For fittings notification handling, the service must decide whether to convert errors into lifecycle events and then return success to transport. The scope should define a wrapper:

```rust
Broker::handle_plugin_publish_value(...) -> Result<(), BrokerError>
```

and service:

```rust
if let Err(err) = broker.handle... { broker.emit_rejection(...); }
return Ok(dummy_response);
```

### 121. Lifecycle rejection event may reveal canonical ids to subscribers

Section: Out of scope lifecycle rejection event.

Payload includes `{ canonical, topic, reason }`. Any plugin subscribed to `core.lifecycle.**` sees canonical plugin ids and rejected topics. That may be acceptable; canonical ids are not secrets. But state it as intentional observability.

### 122. No schema for `reason`

Section: lifecycle rejection.

If `reason` is a free string, tests are brittle and downstream code cannot branch. Prefer structured reason code plus message:

```json
{ "code": "publish_on_reserved_namespace", "message": "..." }
```

At minimum define exact strings.

### 123. `BrokerError` variants should probably include topic on `InvalidPayload`

Section: B2.

Payload decode failure may occur before topic is known. But body shape violations after decode might know topic. Fine. Not essential.

### 124. `InvalidTopic { reason: String }` loses structured `ValidationError`

Section: B2.

m1 has typed `ValidationError`. Broker could store reason string to avoid coupling, but tests should not assert exact text unless specified. Consider source enum or `reason: ValidationError` if cloneable. Current string is acceptable but less precise.

### 125. `BrokerError` should derive `PartialEq`? Tests need matching

Section: E.

Tests will likely `matches!` variants. No need for `PartialEq` if source errors not comparable. But for pure broker errors, deriving `PartialEq` may help. Not blocking.

### 126. `SpawnError` with `anyhow::Error` cannot derive `PartialEq`; tests must use pattern matching

Section: SP2.

Fine, but test descriptions should say match variant, not compare whole error.

### 127. `Broker::register_plugin` duplicate semantics with stale closed peer undefined

Sections: B1/SP5.

If a plugin's peer is closed but guard not dropped, duplicate registration returns `AlreadyRegistered`. If serve loop detects closed, does it drop guard? Not specified. See findings #55/#56.

### 128. `Broker::publish_core` excludes publishing plugin from own fanout only for plugin publishes

Section: B7.

For core publishes, there is no publishing plugin to exclude. Scope should define that `publish_core` fans out to all matching plugins.

### 129. Pattern matching over `auto_subscribes` exact topics should use same matcher or exact compare

Section: B7.

`auto_subscribes` are topic literals, not patterns, but B7 unions them with subscribe patterns and runs pattern matcher. Since topic literals are valid patterns, this works. Good. State that auto-subscribes are exact-topic patterns.

### 130. `auto_subscribes` being included in observer grants may accidentally allow tool_request observation

Section: B7/tests.

Every plugin auto-subscribes to its own tool_request. It does not subscribe to others. Fine.

### 131. `BrokerAcl::tool_routes` is ignored in m2

Sections: Inputs/B1.

m1 `BrokerAcl` includes `tool_routes`, but m2 broker scope does not mention it. Since tool dispatch is out of scope, ignoring it is okay. State that `Broker` stores it but m2 does not use it, or that `BrokerAcl` is consumed whole and tool routes are reserved for m4.

### 132. Provider `bindings.provider = true` ignored by m2 but existing validation allows provider publishes

This reinforces finding #10. It needs explicit staging text.

### 133. `assert_cmd` version not in workspace dependencies style

Section: W1.

W1 says add dev-deps `assert_cmd = "2"` but W1 list is workspace dependencies. Decide whether `assert_cmd` belongs under `[workspace.dependencies]` or directly in `rafaello-core` dev-dependencies. Keep workspace tidy.

### 134. `tokio` features include `process` but supervisor uses std/lockin child, not Tokio process

Section: W1.

May still be useful for tests/fixture. Not harmful.

### 135. `nix` version may conflict with lockin's version

Section: W1.

Scope chooses `nix = 0.27`. Check workspace consistency. Not a blocker, but implementation should avoid duplicate major versions if other crates already use nix.

### 136. `fittings-wire` may not be directly needed by rafaello-core

Section: W1.

If only using `JsonRpcId`, use fittings-core. If using wire types/codec directly, include wire. Given server/client handle codec, broker probably does not need wire. Fixture maybe not either. Avoid unnecessary deps.

### 137. The scope says no top-level workspace deps beyond W1, but adding serial/temp-env may violate it

Sections: W4/Risk #4.

If env tests need `serial_test` or `temp-env`, W4 must be updated. Do not leave this for commits.

### 138. `PluginSupervisor::new(broker: Arc<Broker>)` conflicts with B1 `Broker::new` returning cloneable handle

This reinforces finding #87.

### 139. `Broker` public API lacks introspection needed by tests

Sections: broker_register_plugin test.

The test wants to assert dropping the guard removes registration. It can call publish after drop, but if the plugin is in ACL and not registered, error must be `NotRegistered`. Alternatively expose `is_registered` test-only/public. Current test relies on overloaded NotInAcl.

### 140. `Broker::handle_plugin_publish` after drop cannot fan out anyway because publisher not registered

Section: broker_register_plugin.

Should a plugin need to be registered to publish? Yes, because connection identity should be live. But `handle_plugin_publish` receives canonical directly and can check ACL even if not registered. The scope should state it also requires live registration. Otherwise a stale service with canonical could publish after guard drop.

### 141. Service closure can outlive registration guard

Sections: SP3/SP5.

If serve loop holds canonical and broker clone, and registration guard is dropped, inbound messages already queued could call `handle_plugin_publish`. That function must check live registration, not only static ACL, or stale services can publish after unregister.

This supports adding `NotRegistered`.

### 142. `AlreadyRegistered` should include existing peer? No

No need. Current variant with canonical is fine.

### 143. `core.lifecycle.publish_rejected` from a rejected publish by unregistered plugin?

Sections: B2/lifecycle rejection.

If `handle_plugin_publish` gets a canonical not registered, should it emit a rejection event? There may be no trustworthy live publisher. Probably return `NotRegistered` and do not emit lifecycle event, or emit with canonical if authenticated service still exists. Define.

### 144. `publish_core` payload validation absent

Section: B1/B4.

Plugin publish payload can be any JSON value. Core publish payload also any JSON value. Fine for m2 except lifecycle event schema.

### 145. `PublishMsg` `payload: Value` including null conflicts with overview "payload kind-specific JSON object"

Similar to finding #95. If m2 allows any for generic broker, document as m2 lower-level bus primitive.

### 146. `bus.publish` method params shape with missing params at fittings layer

Section: B4.

If plugin sends `bus.publish` notification with no params or params null, serde decode fails. Covered by `InvalidPayload` if decode handled by broker/service. Add a negative test maybe.

### 147. `bus.publish` with params array should be rejected

Section: B4.

Covered by decode failure but not tested. Optional.

### 148. `in_reply_to` field should maybe deny duplicates

Section: B6.

For exactly-one classes duplicates impossible if arity enforced. For future optional superset, duplicates matter later. Not m2.

### 149. `taint` field should maybe deny empty source

Section: B1.

`TaintEntry { source: String }` has no validation. Since m2 treats plugin taint as untrusted roundtrip, okay. But if any tests or docs display it, invalid/empty source can appear. State m4 validates.

### 150. `PluginSupervisor` registry `Vec<SpawnedPlugin>` cannot remove by canonical efficiently

Not a blocker for m2. Fine.

### 151. Shutdown consuming supervisor should drop in reverse spawn order?

Not specified. Probably not important.

### 152. Fittings serve loop JoinHandle output type is not specified

Section: SP3/SP5.

Store `JoinHandle<Result<(), FittingsError>>` if spawning `server.serve()`. On abort/drop, handle errors. Tests may need logs. Not major.

### 153. `FittingsBuild` error for building Server/Client is probably not a distinct phase

Section: SP2.

`Server::new` is not fallible if transport object already exists. `Client::connect` is fallible because connector connects. Core side likely has no `FittingsBuild` fallible constructor unless transport conversion fails. The error variant may be unnecessary or should cover fd-to-transport conversion.

### 154. `Socketpair { source: nix::Error }` type may be wrong for nix 0.27

Depending on nix version, errors may be `nix::errno::Errno` rather than `nix::Error`. Verify. Do not bake wrong type.

### 155. `EntryNotExecutable` before command means lockin `command` errors are not tested for missing syd

Section: SP2/Risk #5.

`SandboxBuilder::command` can fail if syd missing on Linux. `Lockin` variant should catch `anyhow::Error`. Tests outside devshell may fail here. See finding #61.

### 156. `ProxyStart` before lockin command means proxy may start even if entry invalid unless order preserved

SP3 checks entry executable at step 6 after proxy start step 3. That means an invalid entry with proxy plan starts a proxy then errors. Unwinding should drop it. Better check entry executable before starting proxy to avoid work and side effects. The scope says step 6 for typed error before builder consumed, but nothing prevents doing it before proxy. Move executable precheck earlier.

### 157. SP3 order starts proxy before validating env reserved vars

SP6 reserved env check should occur before allocating resources. Current SP3 env application is step 7 after proxy, builder, command. If plan has reserved env var, scope says return `SpawnError::Internal`; this should happen at step 1.5 before socketpair/proxy.

### 158. SP3 order starts proxy before socketpair? Actually socketpair first

Socketpair before proxy is okay. But all cheap validation should happen before fd/proxy allocation.

### 159. SP3 says steps 1-9 synchronous but step 3 async

Section: SP3.

The text says:

> Steps 1–9 are synchronous. Steps 3 + 10–13 use tokio.

This is self-contradictory because step 3 is inside 1-9 and async.

Rewrite:

- Step 3 is async.
- Step 10+ serve/task parts are async.

### 160. `outpost_proxy::start(policy).await` requires async context before spawn

Covered by making `spawn` async.

### 161. `SandboxBuilder` methods consume self, but loops need reassignment

Section: SP3 step 4.

Lockin builder methods like `read_path` take `self` and return `Self`. Implementation must reassign in loops:

```rust
builder = builder.read_path(path);
```

The scope says "call the method"; fine. Just beware `inherit_fd` vs `inherit_fd_as` ownership styles differ.

### 162. `inherit_fd_as` consumes builder, unlike `inherit_fd` mutably borrows

Section: SP3 step 5.

Need reassignment:

```rust
builder = builder.inherit_fd_as(child_fd, RFL_BUS_FD_NUMBER);
```

This relates to finding #24.

### 163. `command.spawn()` returns `std::io::Result`, not lockin error

Section: SP2.

Scope correctly has `Spawn { source: std::io::Error }` for spawn. Good.

### 164. `SandboxedChild::kill` is not async and may not reap

Covered by lifecycle findings.

### 165. `SandboxedChild::into_parts` could be needed for Tokio waiting but not scoped

If using blocking `std::process::Child`, waiting in async context needs `spawn_blocking` or `tokio` feature of lockin. Lockin crate has optional `tokio` module/feature. W1 does not enable lockin `tokio` feature. Decide sync vs async child management.

### 166. W1 lockin dependency does not enable `tokio` feature

Section: W1/SP1 lifecycle.

If supervisor wants async wait/kill using `lockin::tokio`, dependency must enable `features = ["tokio"]`. The scope cites only sync API. If using sync child with async supervisor, plan for `spawn_blocking`/nonblocking wait.

### 167. `tokio::process` feature in W1 does not help with `std::process::Child`

Unless converting through `into_parts` and rebuilding `tokio::process::Child` is possible, which is not straightforward. Pick a process model.

### 168. `shutdown(self)` with sync child wait can block runtime

Covered by findings #28/#29/#165.

### 169. Plugin-to-core `peer.call` test requires service injection and response id handling

Covered by finding #117.

### 170. Client-side fixture needs notification handler for `bus.event`

Sections: F3/harness.

Current fittings `Client` can `subscribe_notifications` or `with_notification_handler`. Fixture observer must use one of those. Scope says it uses a `Server` for inbound requests but not a notification handler. Since `bus.event` is a notification, `with_service` alone is not enough. The fixture must subscribe/handle notifications.

### 171. Core-side service handling notifications must account for `Request.id = None`

Sections: SP3 step 11.

Fittings represents notifications as `Request { id: None, ... }`. The service should not unwrap id. For notification responses, fittings drops returned response. Implementation detail but tests can catch.

### 172. `Client::with_service` handles inbound requests, not notifications

Section: F3.

F3 says fixture registers a fittings `Service` impl to handle `core.fixture.echo` requests. That is correct for requests. But it also needs notification handling for `bus.event`. Do not conflate.

### 173. `PeerHandle::call` cancellation behavior may send cancellation notifications on dropped futures

Not relevant to m2 tests unless calls timeout/drop. Fine.

### 174. `PeerHandle::closed()` could be useful for lifecycle tests but not mentioned

Not blocking.

### 175. `Broker::publish_core` tests need a peer that receives notifications; using `PeerHandle` directly may require driving a serve loop

Section: broker_publish_core.

A `PeerHandle` queues outbound notifications to its connection's serve/client loop. If tests construct a detached handle without a dispatcher, `notify` may enqueue but nothing observes. Broker tests should either inspect the receiver behind a test PeerHandle or run real fittings connection. Scope does not define.

### 176. `ServiceContext::detached` leaks a receiver intentionally; do not use for broker tests expecting observation

Current fittings has `ServiceContext::detached` for tests but it discards notifications. Not suitable for bus fan-out observation.

### 177. The scope's `BrokerError::Internal` for lock poisoning should not expose poisoned locks as security decisions

Minor. Log and fail internal.

### 178. `core.lifecycle.publish_rejected` on invalid payload without topic lacks topic

If decode fails before topic, rejection payload cannot include topic. The scope's payload says `{ canonical, topic, reason }`. Need `topic: Option<String>` or separate malformed-publish rejection.

### 179. Invalid unknown fields rejection event cannot include attempted topic if serde rejects before extracting topic

Same as finding #178. If you want topic in rejection, parse permissively first or use raw Value to extract `topic` best-effort.

### 180. `InvalidPayload` currently has no canonical in B2

`InvalidPayload { reason }` lacks canonical. Rejection event needs canonical. The function has canonical parameter. Error variant maybe not need it. Fine.

### 181. `BrokerError::InvalidTopic` lacks canonical

Core publish invalid topic has no canonical; plugin publish does. Fine. Rejection event can add canonical from call context.

### 182. `PublishOnReservedNamespace` includes canonical and topic but not namespace

Fine.

### 183. `PublishOutsideGrant` exact set should include sorted lists for error? Not necessary.

### 184. Test names use `broker_publish_core_namespace_rejected` for plugin publish

Section: negative test table.

The file name is misleading. Rename to `broker_plugin_publish_core_namespace_rejected.rs` or change contents to test `publish_core` rejection.

### 185. Positive test `broker_register_plugin` says `handle_plugin_publish` after guard drop returns `NotInAcl`; should be `NotRegistered`

Covered by finding #11.

### 186. Positive test `supervisor_spawn_fixture` says observer asserts publish authority canonical id but event schema lacks it

Covered by finding #13.

### 187. Positive test `supervisor_bus_publish_round_trip` subscribes fixture B to A's topic but B's fixture behavior for ack is not defined

Covered by finding #34/#74.

### 188. Positive test `supervisor_proxy_starts_for_proxy_plan` cannot observe call count or builder call

Covered by findings #107/#108.

### 189. Positive test `supervisor_env_pass_set_applied` requires env dump fixture behavior not defined

Covered by finding #34.

### 190. Positive test `supervisor_private_state_dir_writable` requires fixture path knowledge not defined

Covered by finding #35.

### 191. Negative test `supervisor_spawn_canonical_not_in_acl_refused` suggests process-table observation for no spawn

Section: negative tests.

If spawn returns before process allocation at step 1, no process exists. Process-table observation is hard unless using fixture side effects. Better assert no proxy started and no child handle created via injected spawn/proxy factories, or keep it as unit-level check.

### 192. Negative test `supervisor_spawn_entry_not_executable` must bypass m1 compile

m1 manifest/compile may reject missing/non-file, but not exec bit. Test can hand-mutate. Good. State that it is a hand-constructed plan.

### 193. Negative test `supervisor_reserved_env_in_set_refused` expects `SpawnError::Internal` missing from enum

Covered by finding #19.

### 194. `manual-validation.md` path not specified

Section: Manual validation.

Acceptance says `manual-validation.md` records items. It should specify path, presumably `rafaello/plans/milestones/m2-broker-spawn/manual-validation.md`.

### 195. `retrospective.md` path not specified

Acceptance says `retrospective.md` is written. Specify path in milestone dir.

### 196. `cargo doc --no-deps` warning-free may require documenting public errors/types

Fine. Keep.

### 197. `RUST_LOG=rafaello_core=debug` manual run requires tracing subscriber somewhere

Section: manual validation.

m2 library uses `tracing::warn!`, but no binary initializes subscriber. Tests with `--nocapture` will not show tracing unless test initializes a subscriber or uses `tracing-test`. Manual validation expecting readable registration trace needs a subscriber setup in tests or fixture. Scope does not include `tracing-subscriber` dependency.

Either add test-only tracing subscriber dependency/setup or remove the expectation.

### 198. Fixture has "no tracing setup", but manual validation wants `RUST_LOG` traces

Sections: F4/manual validation.

F4 says no tracing setup in fixture. Manual validation wants `RUST_LOG=rafaello_core=debug` for core traces, not fixture traces. Still needs subscriber in test/core. Clarify.

### 199. `tracing = "0.1"` alone does not emit logs

Section: W1/manual validation.

Need `tracing-subscriber` dev-dependency or test harness subscriber if manual logs matter.

### 200. The scope should not say "No new behavior lands in rafaello-bin" while adding workspace fixture binary without clarifying distribution

It does clarify fixture is not installed. Fine.

### 201. New fixture workspace member under `crates/fixtures/...` changes workspace members glob manually

Section: W3.

Current `rafaello/Cargo.toml` workspace members are explicit:

```toml
members = ["crates/rafaello", "crates/rafaello-core"]
```

Need add `crates/fixtures/rfl-bus-fixture`. Scope says this. Good.

### 202. Fixture crate path under `crates/fixtures/...` may interact with workspace package naming

Fine.

### 203. `publish = false` for fixture is right

Good.

### 204. `README pointer back to m2 scope` is fine but low value

Non-blocking.

### 205. The scope says no `rfl chat` until m3 but m3 milestone expects local TUI as frontend principal

Not an m2 issue.

### 206. `frontend.*` rejection in m2 is correct because no frontend registration path exists

Good, but future-proof error shape. See finding #72.

### 207. `provider.*` rejection in m2 is staged and must be made explicit

Covered by finding #10.

### 208. Cross-plugin RPC out-of-scope but `rpc_reply` is enforced

Sections: B6/out of scope.

The scope enforces `plugin.<id>.rpc_reply` `in_reply_to` even though cross-plugin RPC routing is out of scope. That is okay as a reserved event class. But if no plugin can legitimately publish `rpc_reply` unless it has a grant, tests must include a grant for that topic. Ensure test lock grants it.

### 209. `plugin.<a>.rpc_call.<b>` example from security RFC is not v1 design per out-of-scope

The security RFC table includes `plugin.<a>.rpc_reply`, while m2 out-of-scope says cross-plugin RPC routing not in v1. This is existing plan tension. m2 should not deepen it. Enforcing `rpc_reply` arity is harmless if topic is otherwise granted.

### 210. `provider.<id>.assistant_message` deferral is fine

No issue.

### 211. `frontend.<id>.confirm_answer` deferral is fine

No issue.

### 212. `plugin.<id>.progress` optional in_reply_to not enforced in m2 is fine

No issue.

### 213. `plugin.<id>.*` optional superset rule deferred to m4 is fine if taint is treated untrusted

No issue.

### 214. `BrokerAcl::compile` currently validates provider publishes; m2 broker rejecting them may make some compiled locks unspawnable for provider plugins

Covered by finding #10. Tests should include non-provider fixture only.

### 215. `session.provider_active` irrelevant in m2

Fine.

### 216. `tool_routes` ignored in m2 can be out-of-scope

Covered by finding #131.

### 217. No m2 lazy-load orchestrator is right

Good.

### 218. No helper spawn path is right

Good.

### 219. SP6 checking `RFL_HELPER_FD` is good defense-in-depth but must align with m1 scrubber claim

Covered by finding #23.

### 220. macOS skip wording says `#[cfg_attr(target_os = "macos", ignore)]` but Linux-specific tests should maybe use `target_os = "linux"`

Section: Out of scope macOS.

If non-Linux/non-macOS targets exist, tests may run and fail. Better use:

```rust
#[cfg(target_os = "linux")]
```

for spawn-bearing tests or ignore all non-Linux.

### 221. Tests relying on `/proc` should be Linux-only, not just macOS ignored

Section: manual validation/lifecycle.

Same as finding #220.

### 222. The scope says lockin compiles on macOS but tests rely on syd for Linux

syd is Linux-specific. Fine.

### 223. `nix develop --impure` manual validation is good

Keep.

### 224. Acceptance says `cargo build -p rfl-bus-fixture` without manifest path

Section: Acceptance.

From repository root with multiple workspaces, `cargo build -p rfl-bus-fixture` may not find the rafaello workspace unless root has a workspace including it. Current root may have a dist workspace but check. Safer:

```sh
cargo build --manifest-path rafaello/Cargo.toml -p rfl-bus-fixture
```

### 225. Manual validation `find rafaello/crates/rafaello-core/src -name '*.rs'` is fine

Good.

### 226. The internal split's first item says "~2 commits" for one commit

Section: Internal split.

It says "one commit" then "~2 commits". Minor, but clean it up.

### 227. Internal split says pattern matcher foundational despite already existing

Covered by finding #6.

### 228. Internal split broker registration before publish is fine

Good.

### 229. Internal split supervisor scaffolding includes env tests before fixture dump behavior is scoped

Covered by finding #34.

### 230. Internal split lockin denial proofs after lifecycle may depend on fixture protocol earlier

Fine if fixture grows incrementally.

### 231. Risk #2 says correlator-id collisions are risk; good

No issue.

### 232. Risk #3 says m2 tests do not exercise real CONNECT; this is too weak if proxy support is in scope

Covered by findings #26/#108.

### 233. Risk #4 punts env test serialization; choose now

Covered by finding #60.

### 234. Risk #5 conflicts with acceptance outside devshell

Covered by finding #61.

### 235. Risk #6 child fd snapshot "exactly 0/1/2/3" is bad

Covered by finding #32.

### 236. Risk #7 falsely says only constructor via compile_plugin

Covered by finding #83.

### 237. Acceptance drift item says provider role grants provider authority is m4, but m1 already has provider_id in ACL

Covered by finding #10.

### 238. Acceptance drift item for `bus.event` method name is good

The method-name asymmetry should be recorded. Keep.

### 239. `bus.event` vs `bus.publish` naming is acceptable

No blocker. It is asymmetrical but clear.

### 240. `bus.subscribe` out-of-scope is right

Good.

### 241. No runtime subscribe path means broker must never accept `bus.subscribe` request in m2

Add explicit method rejection test maybe. Not blocking but useful.

### 242. `PeerHandle::call` core-to-plugin round-trip is a good test

Keep once fittings construction is corrected.

### 243. Plugin-to-core `peer.call` is a good test but needs service injection

Covered by finding #117.

### 244. `supervisor_bus_publish_round_trip` is a good topology test but currently underspecified

Covered by findings #21/#34/#74.

### 245. `broker_publishing_plugin_excluded_from_own_fanout` is important

Keep.

### 246. Self-fanout exclusion should happen after authorisation but before delivery

Section: B7.

Fine. If publisher not registered, no fanout. If core publish, no exclusion.

### 247. Excluding publisher may hide bugs for self-observability

Security choice is okay. Scope explicitly chooses no echo. Good.

### 248. `auto_subscribes` self-subscribe plus no echo means publishing own tool_request would not return to self even if allowed

Further supports rejecting publish-on-auto-subscribe.

### 249. `Broker::publish_core` should not exclude any plugin named core

No issue.

### 250. `PublishOnReservedNamespace` for `provider.*` may be semantically wrong once provider support lands

Future issue. For m2 okay if explicit.

### 251. The scope should add a `UnknownNamespace` error variant or map to reserved

Sections: B2/B3.

Given finding #7, choose error shape. `PublishOnReservedNamespace` is not accurate for `evil.foo`. Add:

```rust
UnknownNamespace { canonical, topic }
```

or broaden reserved to "PublishOutsideNamespace".

### 252. `PublishOutsideGrant` for `plugin.<own>` short topic may be okay

If topic grammar valid but not in grant, this can be `PublishOutsideGrant`. For unknown top-level, use unknown namespace.

### 253. `validate_topic` allows top-level `evil`, by design

Grammar and namespace are separate. Broker must enforce namespace.

### 254. m1 manifest validation allows publish topics outside known namespaces except core/frontend? Need check

Current `check_publish_topic` rejects `core` and `frontend`, allows unknown top-level at manifest standalone. `manifest_with_id` catches plugin/provider mismatches but unknown top-level may pass. Lock validation `check_lock_publish_topic` also allows unknown top-level. This means m1 may produce grants for `evil.foo`. m2 broker must reject unknown namespace at runtime, and m1 may need retrospective fix. The scope does not mention this.

This is significant: the unknown namespace hole is not only in m2 B3; m1 validation may have let it through. m2 should add runtime rejection and retrospective drift item for m1 validation.

### 255. `BrokerAcl::compile` currently validates grammar but not known namespace beyond m1 lock validation

Same as finding #254.

### 256. `compile_plugin` currently sets publish/subscribe vectors empty

Covered by finding #50. If supervisor tests inspect `plan.subscribe_patterns`, they will fail.

### 257. The harness says wraps `compile_plugin` and `broker_acl::compile`; good

Use `broker_acl` for broker authority.

### 258. Hand-mutating canonical to not in ACL may leave topic_id inconsistent

Section: supervisor_spawn_canonical_not_in_acl_refused.

If test mutates `CompiledPlugin.canonical` but not `topic_id`, supervisor should reject before using topic_id. Good. Keep as early check.

### 259. Mutating canonical in a public struct demonstrates risk #7; do not downplay

Covered.

### 260. `Broker::register_plugin` should probably verify topic_id in ACL matches plan.topic_id? Not possible from register alone

Supervisor can compare `plan.topic_id` to broker ACL's topic id for canonical as defense in depth. This would catch mutated plans. Scope does not include it. Add it if keeping strong plan-integrity claims.

### 261. Supervisor should compare plan publish/subscribe fields? Since current compile_plugin empty, no

Better ignore those fields or remove from CompiledPlugin in later cleanup.

### 262. Supervisor should verify reserved env before `env_clear` application

Covered by finding #157.

### 263. `RFL_PLUGIN` canonical string may contain characters okay for env value

Fine.

### 264. Env `pass` missing keys skipped is okay

Good.

### 265. `env.set` overriding `pass` is okay

Good.

### 266. Reserved env injection before pass/set with set override would be unsafe, but scope injects reserved then pass/set? Check SP3 step 7

SP3 says:

1. `env_clear`
2. Inject reserved vars
3. pass
4. set

If a reserved var somehow appears in pass/set and SP6 check failed, pass/set would override reserved. Since SP6 is defense-in-depth, do it before env application. Also safer order is pass/set first, then reserved last, even with checks. The scope currently injects reserved before user env. Change to inject reserved after pass/set or keep check as hard guarantee. Security-conservative: reserved last.

### 267. m1 compile says set overrides pass; reserved vars should override both regardless

Covered by finding #266.

### 268. `env_clear` plus pass looks up parent env at spawn time; good

No issue.

### 269. Secret env scrubber already ran in compile, but spawn trusts plan

SP6 catches only reserved. A hand-mutated plan could pass `OPENAI_API_KEY` despite scrubber. Risk #7 says best effort. Fine if claim weakened.

### 270. `RFL_HELPER_FD` reserved even though helpers deferred is good

Yes, but update m1 or state m2-only.

### 271. No helper spawn path is essential

Good.

### 272. `allow_interactive_tty` not called is good

Good.

### 273. `raw_seatbelt_rule` not called is good

Good.

### 274. `allow_kvm` not called is good

Good.

### 275. `allow_non_pie_exec` not called may break some binaries but is conservative

Covered by finding #79.

### 276. Need an explicit fixture lock grant for executing fixture

Lockin automatically makes program executable, but if fixture launches no subprocesses, no `exec_paths` needed. State this.

### 277. Fixture may need read access to dynamic libs

Lockin backend handles path candidates/system loader paths. Fine.

### 278. Private state dir may need to be created before spawn

Sections: compile private state, supervisor_private_state_dir_writable.

m1 compile adds read/write dirs but does not necessarily create the directory. If lockin grants a non-existent write dir, can child create it? Depends on parent directory permissions. The test expects writing marker under the dir. Supervisor should ensure private-state directory exists before spawn or m1 compile/test harness should create it.

The scope does not mention directory creation. Add it.

### 279. If private state dir does not exist, write to marker fails despite correct grant

Same as finding #278.

### 280. Filesystem write grant to private state may require read/stat on parent `.rafaello-plugin-data`

Lockin path candidates likely add traversal stat/read. But directory creation still matters.

### 281. Current m1 compile adds private state to both read_dirs and write_dirs

Good.

### 282. Supervisor should not create arbitrary write_dirs

Only private state maybe. User-granted write dirs may already exist or not? If granting a file path, maybe not. Scope should not create all write dirs. For private state, core owns it.

### 283. `FilesystemPlan` does not identify which write dir is private state

Supervisor can derive from topic/project root only if it has project root. It does not. Another reason private state creation may belong to compile/test harness or plan should include it.

### 284. `CompiledPlugin` lacks project_root

Sections: private state/current_dir.

m1 `PathContext` had project root, but `CompiledPlugin` does not retain it except embedded absolute paths in filesystem grants. Supervisor cannot know project root from plan except heuristics. If m2 needs project root for cwd/private state env/tests, add a field or pass context to supervisor.

### 285. Private state test can use absolute path from plan.write_dirs instead of PROJECT_ROOT

Fixture could be given the exact path via env set. That avoids adding project root to supervisor. Scope should choose.

### 286. The scope says private state path uses topic-id; m1 already tests that

m2 integration test still useful to prove sandbox write, but do not duplicate too much.

### 287. `outpost::NetworkPolicy::from_allowed_hosts` error type not std::io

Compile uses it at m1 dry-run. Supervisor uses it again? Step 3 synthesises and calls; if `from_allowed_hosts` can fail, `SpawnError` lacks a variant except ProxyStart io. Since m1 compile already dry-ran, invalid hosts indicate mutated plan. Add `InvalidAllowHosts`/`Internal` or unwrap with internal error.

### 288. `NetworkPlan::Proxy { allow_hosts }` from compiled plan already validated by m1

But hand-mutated plan risk. Decide defense-in-depth.

### 289. `outpost_proxy::start` rejects policy default_action Log with io error?

Not important.

### 290. The proxy handle must outlive child

SP5 says `SpawnedPlugin` drops proxy handle. Good.

### 291. Proxy env should point to `http://127.0.0.1:<port>` or listen addr host

Scope should specify exact env values and IPv4/IPv6 behavior. Lockin CLI uses loopback URL. Reuse it.

### 292. `ProxyHandle::listen_addr()` may be IPv4 loopback

Use port for lockin `network_proxy`. If listen addr is IPv6, `network_proxy(u16)` only takes port and lockin allows loopback port. Fine.

### 293. The scope should avoid asserting proxy call count without injection

Covered.

### 294. `NetworkPlan::AllowAll` carried because plan supports it; okay

Good.

### 295. m2 tests for AllowAll? None

Not required if out-of-scope risky override. But at least unit test builder mapping? Optional.

### 296. `NetworkPlan::Deny` default with inherited AF_UNIX fd is correct

Good.

### 297. Security RFC says proxy mode denies AF_UNIX outbound; m2 inherited fd bypass is intended

Good.

### 298. The scope's claim m2 closes attack 6.5 subscribe unauthorized depends on no runtime subscribe

Good: broker only fans out based on ACL. Need tests for plugin not receiving unsubscribed topics. The matrix has positive subscribed receipt but no explicit negative unsubscribed drop except rejection cases. Add a test: plugin B not subscribed does not receive A's event.

### 299. Current no-fanout tests maybe cover no original delivery on rejection

Not enough for subscribe ACL drop.

### 300. `Broker::publish_core` fan-out test should include unsubscribed plugin drop

Optional but valuable.

### 301. No test for duplicate registration direct broker

`broker_register_plugin` should include duplicate. Scope says errors include AlreadyRegistered but test happy path only. Add negative direct broker duplicate.

### 302. No test for `Broker::register_plugin` canonical absent direct broker

Negative supervisor covers spawn not in ACL, but direct broker NotInAcl should be tested.

### 303. No test for fan-out notify error non-fatal

Could create closed peer and ensure publish still succeeds. Optional but B7 promises it.

### 304. No test for publish_core invalid topic

Covered by finding #71.

### 305. No test for unknown top-level namespace

Covered by finding #7.

### 306. No test for short plugin topic

Covered by finding #8.

### 307. No test for in_reply_to wrong arity

Covered by finding #43.

### 308. No test for taint roundtrip

Positive publish tests assert payload/topic but not taint. If B1 promises m2 round-trips taint, add a test.

### 309. No test for unknown field inside taint entry

Depends on decision finding #44.

### 310. No test for plugin publishing on own auto-subscribe rejected/allowed

Given B3 contradiction, add explicit test once decision made.

### 311. No test for `bus.publish` with missing payload field

Optional but part of B4 required fields.

### 312. No test for `bus.publish` with `payload: null`

B4 explicitly allows null. Add positive unit test.

### 313. No test for numeric/null JsonRpcId in `in_reply_to`

Since fittings supports string/number/null, add at least numeric. Optional.

### 314. No test for `PublishMsg` unknown top-level field through actual service

There is `broker_publish_extra_field_rejected`, but direct broker API cannot decode. Ensure it tests service/raw decode or change API.

### 315. No test for plugin sending `bus.publish` request rather than notification

Optional but protocol-important.

### 316. No test for unknown method handling

Optional.

### 317. No test for child fd env value exactly `3` from fixture

Env dump test covers `RFL_BUS_FD=3`. Good once fixture behavior added.

### 318. No test that parent-supplied `RFL_BUS_FD` is not passed via env.pass

Reserved env in set test exists; add pass test too. Scope says set/pass both checked.

### 319. No test for `RFL_PLUGIN` reserved in set/pass

Add one or table-driven reserved env test.

### 320. No test for `RFL_HELPER_FD` reserved in set/pass

Since SP6 promises it, test it.

### 321. No test for env.set overriding env.pass

Env pass/set test could include collision. Scope says order matches m1. Add.

### 322. No test for missing env.pass skipped

Optional.

### 323. No test for `env_clear` removing unrelated parent env

Could be tested by dump_env to ensure random parent var not present unless passed. Valuable.

### 324. No test for dynamic linker env stripping

Covered by finding #78.

### 325. No test for proxy env injection

Covered by finding #26.

### 326. No test for private state dir creation

Covered by finding #278.

### 327. No test for process tree cleanup

If claimed, add. If not, weaken lifecycle claim.

### 328. No test for duplicate supervisor spawn cleanup

Covered by finding #53.

### 329. No test for spawn failure after child spawn but before registration

`drop_during_spawn_unwinds` tries but uses wrong failure. Need a real injected failure. Without dependency injection, hard to force fittings build failure after child spawn. Consider adding test-only hooks or accept less coverage.

### 330. fd-count before/after tests are flaky under parallel Tokio

Section: `supervisor_drop_during_spawn_unwinds`.

Counting `/proc/self/fd` before/after in async tests can be noisy because other tests run in parallel and Tokio opens fds. Use serial tests or avoid fd-count assertions. If using fd counts, mark serial and isolate.

### 331. Process-table observation is flaky under PID reuse

Lifecycle tests using `kill -0` after bounded wait can race with PID reuse. Better hold child handle and wait/reap. If only PID, use start time from `/proc/<pid>/stat` to avoid reuse. Simpler: supervisor exposes wait.

### 332. Tests need `serial_test` if they inspect global process/env/fd state

Covered by finding #60/#330.

### 333. The fixture should probably support a readiness call

Covered by finding #21.

### 334. The scope does not specify max frame size for fittings transport

If using `StdioTransport::new(..., max_frame_bytes)`, choose a size. Existing examples may have defaults. Scope should include it or use a transport with default. Tests can use small payloads.

### 335. The fixture should handle malformed `RFL_BUS_FD` exit code

Not necessary for m2 tests, but hard failures eprintln per F4.

### 336. The fixture should not log too much; F4 is good

Keep.

### 337. `publish_bad_namespace` fixture "do not exit on error" cannot know error; fine

It just sends notification and continues. Good.

### 338. Fixture publish on startup should wait until client loop is running

If using `Client::connect`, once connected worker spawned. Fine. But core observer readiness still issue.

### 339. Core side server must be spawned before fixture client can communicate

If child publishes before core serve loop, socket buffers. Fine if registration before serve. Need race contract.

### 340. `Broker::register_plugin` before `server.serve` means outbound `bus.event` can be queued before serve drains

If core publishes immediately after spawn returns but before serve loop scheduled, `peer.notify` queues into server's channel. Once serve starts, it sends. Fine.

### 341. If serve loop never starts, broker thinks plugin registered and notify queues until channel fills

Spawn should spawn serve loop before returning. It does step 13 before return. Good.

### 342. Register before serve loop means inbound queued publishes are not processed until after registration

This can satisfy race if transport not read until serve. State that.

### 343. Step 12 register before step 13 serve loop is correct for inbound auth

Good. The false part is "before child has chance to publish".

### 344. `Server::peer()` can be called before `serve`; current API supports this

Good.

### 345. `Client::peer()` in fixture can be used after connect

Good.

### 346. Need connector implementation for inherited fd on fixture side

Sections: F3.

`Client::connect` takes a `Connector`, not a raw transport. The fixture needs a connector that returns the Unix fd transport exactly once. Scope says uses fittings-core to build Client; should specify connector or use lower-level API if any.

### 347. Core side `Server::new(service, transport)` takes a transport directly, so no connector needed

Good.

### 348. If using `StdioTransport` with split UnixStream, writer and reader halves types must be Send + Unpin

Likely yes. Implementation detail.

### 349. `UnixStream::from_std` requires a connected Unix stream from raw fd

Socketpair gives that. Good.

### 350. `nix socketpair` returns `OwnedFd` with current Rust/nix? Verify

nix 0.27 socketpair may return `OwnedFd` depending features/Rust version. Scope assumes OwnedFd. Verify at implementation. If it returns RawFd, convert safely.

### 351. `SOCK_STREAM` framing with newline JSON is okay

Fittings transport handles newline frames. Good.

### 352. The scope does not mention frame backpressure for bus.publish inbound

Fittings max in-flight/notification capacity handles. Not m2.

### 353. B7 says slow consumer drops to sink, not publisher; correct per fittings notification semantics

Good.

### 354. B7 should mention `peer.notify` serializes params via fittings, so `BusEvent` must be serializable

Obvious but include derives.

### 355. `PublishMsg` should derive `Deserialize`; `BusEvent` should derive `Serialize`

Add to B1.

### 356. `TaintEntry` should derive both Serialize and Deserialize if round-tripped

Add.

### 357. `CanonicalId` in event payload needs Serialize if included

If publisher identity uses canonical id, `CanonicalId` must serialize or convert to string. Current type likely has display/parse but check. Event payload can use string.

### 358. Broker rejection event should serialize canonical as string

Probably.

### 359. `BrokerError` variants using `CanonicalId` require Clone/Debug/Error; current CanonicalId likely supports

Fine.

### 360. `SpawnError` variants with `PathBuf` okay

Fine.

### 361. `Broker` should not expose mutable ACL

No issue.

### 362. `RegisteredPlugin` drop should be idempotent

If dropped once only, fine. If internal unregister sees missing due to child watcher, should tolerate. Scope should define if multiple teardown paths exist.

### 363. `RegisteredPlugin` should probably be `!Clone`

RAII guard should not clone. If it must be shared, use inner option. Scope can leave.

### 364. `SpawnedPlugin` should not be `Clone` if it owns child/guard

If public handle is cloneable, separate owned state from handle.

### 365. `PluginSupervisor` Drop should call best-effort cleanup

SP5 implies dropping supervisor kills children. Implement Drop on supervisor, not only SpawnedPlugin, if supervisor owns Vec. Scope should state.

### 366. `PluginSupervisor::shutdown(self)` should prevent Drop double-kill

Use ownership/flags. Implementation detail but mention best effort idempotent cleanup.

### 367. `kill -0` lifecycle test should be replaced with wait/reap

Covered.

### 368. `SIGTERM` grace period default configurable but no API for config

Section: SP1.

It says configurable grace period default 5s, but `PluginSupervisor::new` takes only broker. Need config API:

```rust
PluginSupervisor::with_shutdown_grace(...)
```

or remove configurable claim.

### 369. Tests requiring ≤ grace period will take 5s if default used

Slow. Use shorter test config. Need API.

### 370. `Drop` cannot use configurable async grace anyway

Covered.

### 371. The scope should separate `Drop` best-effort from `shutdown` graceful

Do that.

### 372. `SpawnedPlugin::Drop sends SIGTERM` conflicts with `PluginSupervisor::shutdown` also sending SIGTERM

If shutdown consumes and drops after, ensure no double signal. Fine with idempotence.

### 373. `RegisteredPlugin` guard drop before serve task abort may let queued inbound publishes return NotRegistered

Teardown order. Fine if closing.

### 374. Serve loop abort may drop transport and close core fd

Good.

### 375. Parent core_fd ownership after passing to transport should be moved, not retained separately

SP3 step 9 says parent retains `core_fd` only. Step 10 builds transport from `core_fd`, moving it. After that parent retains transport/serve loop, not raw fd. Rewrite ownership accurately.

### 376. `SandboxedCommand` owns child fd until spawn; after spawn parent no longer has it except command consumed

Covered by finding #24.

### 377. Failure before command.spawn drops SandboxedCommand and closes child fd

Good if ownership modeled.

### 378. Failure after command.spawn must kill child

SP3 says so. Need implementation.

### 379. Failure after proxy start before child spawn drops proxy

Good.

### 380. Failure after registration before storing guard must unregister

Use local guard and move into SpawnedPlugin only at end. Good.

### 381. Failure after serve loop spawn but before storing handle unlikely

Still possible if allocation push fails? Rust aborts on OOM. Not important.

### 382. `Vec<SpawnedPlugin>` push cannot fail recoverably

No issue.

### 383. `Broker::register_plugin` should be called before serve loop starts

Yes.

### 384. But outbound core-to-plugin calls before plugin client is ready may fail/time out

Tests should wait readiness before peer.call. Need handshake.

### 385. `spawn` returning after serve loop spawn does not mean fixture initialized

Need readiness for tests.

### 386. The fixture publish-on-startup may happen before spawn returns

Possible. Observer channel must be ready before spawn publisher.

### 387. Test harness should spawn observer, wait ready, then publisher

Add.

### 388. Observer fixture subscribed to `plugin.<A>.greet` needs A topic id before A spawn

Can compute from canonical. Fine.

### 389. Multiple plugin lock builder must avoid topic-id collisions

m1 validates. Fine.

### 390. Fixture canonical ids should be distinct per instance

If using same fixture binary for multiple plugins, canonical ids must differ. Harness should define names/versions accordingly.

### 391. Duplicate canonical in lock impossible due BTreeMap

Fine.

### 392. Topic-id derivation from canonical version means two fixture instances need different canonical names or versions

Harness should choose.

### 393. `env.set` fixture controls differ per plugin instance

Harness must support per-plugin env.

### 394. `CompiledPlugin.entry_absolute` points at built fixture in target dir outside plugin_dir

m1 compile resolves entry relative to plugin_dir and digest gates plugin content. For a test fixture binary in target dir, creating a lock entry pointing at it may violate m1 manifest entry resolution/package digest assumptions unless the harness creates a fake plugin_dir with entry path/symlink.

The scope says lock entry points at fixture binary path and wraps compile_plugin. But `compile_plugin` resolves `entry_absolute = plugin_dir / entry`. It will not accept arbitrary external target path unless plugin_dir is set accordingly and entry is relative within it.

This is a major harness gap. Options:

- Copy/symlink fixture binary into temp plugin_dir and set entry to that relative path, with digest recomputed accordingly.
- Hand-construct `CompiledPlugin` for supervisor tests, but then the "same path as real plugin" claim weakens.

### 395. Digest gating complicates fixture lock builder

m1 `compile_plugin` requires recomputed content and manifest digests match lock entry. Harness must build valid fake plugin dirs/manifests/digests or bypass compile. Scope says wraps compile_plugin but does not explain digest setup.

Given m1 patterns may exist, still specify enough.

### 396. Fixture binary path outside package may be blocked by entry escape validation

Manifest parse/compile require entry inside package dir. If using `env!(CARGO_BIN_EXE...)` absolute path as entry, m1 will reject/escape. Need copy/symlink strategy.

### 397. Symlink to target binary inside plugin_dir may affect digest and lockin execution

If symlink points outside package, digest walker may reject symlink escape. Copy is safer.

### 398. Copying fixture binary into temp plugin_dir requires executable bit preserved

Harness should do it.

### 399. Fixture binary in temp plugin_dir may need dynamic library paths/Nix store access

The executable path is copied, but dynamic libs in Nix store must be readable. lockin automatically allows loader candidates for program, but not all libraries? It likely observes path candidates. Existing lockin tests handle. Fine.

### 400. `exec_path` inside project refused by m1 may affect copied fixture if plugin_dir under project root

If temp plugin_dir is under project root and entry is there, m1's exec_path refusal applies to grant exec_paths, not entry. Entry okay. But plugin_dir maybe outside project root to avoid confusion.

### 401. The scope says programmatic temp lock/project layout no on-disk fixtures directory; fine

But fixture binary itself must be materialized into temp plugin dirs if using compile.

### 402. `FixtureLockBuilder` needs to produce both `Lock` and `PathContext/RecomputedDigests`

Scope just says wraps compile_plugin/broker_acl. Add details.

### 403. No mention of `openrpc.json` sibling required by m1 manifest

m1 manifest requires openrpc sibling. Harness fake plugin dirs must include it or construct lock directly. Scope does not mention. If using lock entries directly after m1, maybe manifest not needed. But digest/manifest_digest still needed. Clarify.

### 404. No mention of manifest digest for fixture lock entries

Covered.

### 405. `compile_plugin` currently spot-checks V3 and digests; tests hand-mutating lock must run validate

Harness must run `validate::lock` before compile to satisfy precondition. Scope says wraps compile_plugin and broker_acl, not validate. Add.

### 406. `broker_acl::compile` also assumes validate ran

Same.

### 407. Test harness adding observer subscribed to broad patterns may trip trifecta?

If observer has no FS/network/write, bus subscribes/publishes only, likely fine. But if broad subscribe to provider/core events counts untrusted read? Trifecta evaluates FS/network/workspace write, not bus subscribe? Need check. Likely fine.

### 408. Fixture env controls use `env.set`; m1 reserved/secret scrubber may reject some names?

Names `RFL_FIXTURE_*` not secret by patterns except none. Fine.

### 409. `FAKE_API_KEY` in env.pass will be scrubbed by m1 unless `i_know_what_im_doing` true

Important. The env pass/set test says plan with `env.pass = ["FAKE_API_KEY"]` and expects it forwarded. Current m1 scrubber strips `*_KEY` unless `i_know_what_im_doing` is true.

So the test as written will fail if built through compile_plugin with default flags. Either:

- use a non-secret name like `FAKE_PUBLIC_ENV`, or
- set `i_know_what_im_doing = true` in the lock and state that the test covers override behavior.

This is a concrete m1 mismatch.

### 410. `OPENAI_*` and other secret patterns stripped; tests should not use secret-looking names unless testing stripping

Covered.

### 411. Env `set` is not scrubbed for secret patterns, only reserved? Current scrubber rejects reserved and strips pass secrets, set survives. If set contains FAKE_API_KEY it would pass. Test uses pass.

Fine.

### 412. `RFL_FIXTURE_RESPOND_PEER_CALL` env var controls only one method; tests need multiple methods

Fixture may need support for comma/list or multiple vars. Scope currently one var. Add methods or separate vars.

### 413. If fixture both publishes and holds open, order clear

F2 says after behaviours sleep. Good.

### 414. If fixture both opens file and responds to report, it must hold result and stay open

Need `HOLD_OPEN` or service implies hold. Define.

### 415. Fixture default exits after behaviours; for peer.call response tests it must stay alive long enough for test call

If `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.echo`, F2 says register service. Without `HOLD_OPEN`, after performing startup behaviors it exits 0. Then test calling echo may race with exit. For any respond-peer-call mode, fixture should hold open until SIGTERM by default, or tests must set `HOLD_OPEN=1`.

The scope does not say this. Current F2 says `HOLD_OPEN` defaults 0 and fixture exits once configured behaviours complete. Registering a service with no hold means it exits immediately. Core-to-plugin peer.call test will fail.

### 416. Plugin-to-core call fixture exits 0 on successful response; good

That mode should not hold by default. But service modes should.

### 417. Observer fixture must hold open to receive events

Set `HOLD_OPEN=1` or make observer mode hold. Scope should specify.

### 418. Bad publish fixture may exit before rejection event observed

If rejection event goes to observer, fine. Publisher can exit. But broker service must process publish before child exit closes fd. Usually queued. Use readiness/ack.

### 419. `RFL_FIXTURE_PUBLISH_BAD_REPLY_TO` must use actual topic id

Env var value includes `<topic-id>`. Harness can set. Good.

### 420. `RFL_FIXTURE_PUBLISH_BAD_GRAMMAR=plugin.id_xxx.UPPERCASE` topic id may not match fixture own id

If id_xxx is not fixture topic id, broker could reject as foreign namespace before invalid grammar depending precedence. Use own topic id with invalid later segment to test grammar deterministically.

### 421. `plugin.id_xxx.UPPERCASE` has `id_xxx` as valid segment but maybe wrong id

Covered.

### 422. Bad grammar test should include empty topic and spaces as direct broker tests

Scope says sub-cases. Good.

### 423. Fittings request method names can contain dots; fine

No issue.

### 424. `core.fixture.*` as method names not bus topics; clarify to avoid ACL confusion

Covered.

### 425. The fixture should maybe expose observed events through peer.call from core? Inbound notification handler cannot initiate call without peer handle? Client has peer. Yes.

Implementation detail.

### 426. If observer calls core service from notification handler, core service injection needed

Same as plugin-to-core call service injection. Harness needs supervisor service extension.

### 427. Without service injection, observer can write to stdout/stderr and test captures child output

Possible but less clean. Scope says one-shot receiver, so service injection likely.

### 428. `PluginSupervisor` public API lacks hooks for test-side subscriber receiver

Harness may build broker and services directly. But scope says helper returns receiver. Need hooks.

### 429. Maybe use a fake registered PeerHandle instead of observer fixture for broker tests

For supervisor end-to-end, real observer fixture is good. But for many broker tests, fake peer simpler.

### 430. Scope should distinguish unit broker tests from supervisor integration tests

Negative table labels all under integration tests, but many are direct broker API. Clarify.

### 431. `#[tokio::test(flavor = "multi_thread")]` for pure pattern matcher is unnecessary

`bus_pattern_matches.rs` can be normal `#[test]`. Scope says every test is tokio multi_thread. Not harmful but silly. Use normal unit test for pure matcher.

### 432. `bus_pattern_matches.rs` described as unit suite but placed under integration tests

Fine either way. Existing helper already tested maybe. Avoid duplicate.

### 433. Tests parallel schedule deterministic is a good requirement

Yes, but env/fd/process tests need serial.

### 434. `tempfile` already in workspace dependencies; good

No issue.

### 435. `assert_cmd` may be unnecessary if using fixture copy strategy

Covered.

### 436. `manual-validation` requiring doc warning-free is good

Keep.

### 437. `retrospective` anticipated drift list is useful

Good, but add drift for unknown namespace/m1 validation if not fixed in m2.

### 438. The scope says no Stream RFC patches owed beyond listed items; likely false

Given provider namespace staging, lifecycle rejection schema, event method name, unknown namespace validation, and maybe request_id/taint staging, m2 may owe more drift notes. Do not pre-declare no drift owed.

### 439. Decisions row 38 provider rename included in inputs; good

No issue.

### 440. Main docs say helpers deferred but security RFC header still says CaMeL dependencies include helper plugins v1

Existing stale RFC issue. m2 should avoid relying on helper body. Scope mostly does.

### 441. Scope says read §5.7 banners and not introduce frontend surfaces; good

No frontend path introduced. Good.

### 442. Scope says no provider surfaces; good, but provider_id in ACL tension

Covered.

### 443. Scope says no helper surfaces; good

Good.

### 444. Scope says no CLI; good

Good.

### 445. Scope says no session persistence; good

Good.

### 446. Scope says no renderer model; good

Good.

### 447. Scope says no runtime bus.subscribe; good

Good.

### 448. Scope says no lazy-load orchestrator; good

Good.

### 449. Scope says no cross-plugin RPC routing; good

Good.

### 450. Scope still enforces rpc_reply; acceptable if just schema guard

Covered.

### 451. `Broker::handle_plugin_publish` should not route plugin-to-plugin RPC calls by topic

No cross-plugin route. Generic fan-out could still deliver `rpc_call` if granted. If cross-plugin RPC not v1, consider rejecting `plugin.<id>.rpc_call` topics or not granting them. Scope only mentions rpc_reply. Clarify.

### 452. Security RFC attack 6.10 says cross-plugin RPC denied, but m2 generic fan-out could allow arbitrary plugin topics if lock grants subscribe/publish

This is similar to result-routing concern. If plugin A is granted publish on `plugin.<A>.foo` and plugin B subscribes, that is plugin-to-plugin event communication through broker. That is allowed by bus ACL generally. But specific RPC bypass should not exist. Define whether arbitrary plugin event fan-out is allowed in v1. Overview says plugins talk through core-mediated bus; grants can allow. Security says no plugin-to-plugin RPC route bypasses core. Generic events are core-mediated. Fine.

### 453. `plugin.<a>.rpc_call.<b>` would be under A's namespace and could fan out to B if B subscribes

If not v1, m1 should not grant such topics. m2 could reject `rpc_call` reserved class. Scope does not mention. Maybe out of scope.

### 454. `plugin.<id>.tool_request` fan-out from plugin publisher should probably be rejected because only core routes tool requests

This is the auto_subscribe publish issue. A plugin should not publish its own tool_request unless explicitly weird. Conservative reject.

### 455. `plugin.<id>.tool_result` raw event fanout should be restricted

Covered.

### 456. `plugin.<id>.progress` generic fanout okay

Likely.

### 457. `plugin.<id>.hello` fixture topic is fine as generic plugin event

Good.

### 458. Publish grants for fixture hello must include exact topic

Harness must grant.

### 459. Observer subscribe to `plugin.<A>.hello` or `plugin.**` must be granted

Good.

### 460. `plugin.**` pattern is valid and matches `plugin.id.foo` but not `plugin.id`?

Pattern has two segments: `plugin`, `**`; `**` matches one or more trailing segments, so matches any plugin topic with at least one segment after `plugin`, including `plugin.id`. Fine. If wanting all plugin subtopics, `plugin.**` is valid.

### 461. `frontend.**` and `provider.**` valid observer patterns

Yes.

### 462. `core.**` valid observer pattern

Yes.

### 463. Direct `**` invalid

Covered.

### 464. `core.lifecycle.boot` boot topic grammar okay

Good.

### 465. `core.lifecycle.publish_rejected` grammar okay

Good.

### 466. `core.session.user_message` plugin forbidden topic grammar okay

Good.

### 467. `frontend.tui.confirm_answer` plugin forbidden topic grammar okay

Good.

### 468. `provider.openai.tool_request` plugin forbidden in m2 okay if staged

Covered.

### 469. `plugin.<B-topic-id>.tool_result` cross-plugin rejected as reserved/foreign namespace

Good.

### 470. Error name `PublishOnReservedNamespace` for foreign plugin namespace is slightly odd

It says reserved namespace for another plugin. Fine but maybe `PublishOutsideNamespace` clearer.

### 471. Error name `PublishOutsideGrant` for unknown namespace would be wrong

Add unknown namespace.

### 472. `InvalidTopic` reason string should include validation error but not leak internal? Fine.

### 473. `Internal` errors should not be used for validation failures

Reserved env hand-mutated plan is internal assertion maybe okay. But a user could hit if compile bug. Fine.

### 474. SP6 internal detail string exact match in tests is brittle

Negative test expects exact detail. Prefer match variant and contains var name, not exact full string.

### 475. `SpawnError::Internal` should include canonical if plan-specific

SP6 internal could include canonical. Add.

### 476. `SpawnError::Socketpair` lacks canonical

Socketpair happens for a plan; include canonical for context? Current variant only source. Not critical but useful.

### 477. `SpawnError::ProxyStart` includes canonical; good

Yes.

### 478. `EntryNotExecutable` includes path; good

Yes.

### 479. `FittingsBuild` includes canonical; good but type wrong

Covered.

### 480. `Lockin` variant name for command build missing syd may not only be lockin policy

Fine.

### 481. Should `SandboxBuilder::command` missing syd be considered spawn-time environment error not plugin error

Yes. Variant okay.

### 482. `Spawn` variant source `std::io::Error` good

Yes.

### 483. `ProxyStart` source `std::io::Error` good

Yes.

### 484. `Socketpair` source type verify

Covered.

### 485. `EntryNotExecutable` should occur before proxy start

Covered.

### 486. Reserved env check should occur before proxy start

Covered.

### 487. NotInAcl check should occur before proxy start

It does. Good.

### 488. Plan topic_id consistency check should occur before resources

Add if strong integrity.

### 489. Filesystem absolute path assumptions from m1 okay

`CompiledPlugin` paths absolute. Hand-mutated plan could include relative paths and lockin builder panics. Supervisor should either trust compile or spot-check to avoid panics. Scope only spot-checks reserved env and entry executable. Since builder methods panic on relative paths, a malicious hand-constructed `CompiledPlugin` can panic supervisor. If public API accepts public struct, add defense-in-depth path absolute checks or weaken trust claim.

This is important: lockin builder panics on non-absolute paths. m2 should not allow public input to panic core.

### 490. Network allow_hosts from hand-mutated plan can fail from_allowed_hosts

Covered.

### 491. Limits values 0 may have special meanings

m1 preserves explicit 0 for CPU/fds. Applying `max_open_files(0)` may break child. This is granted behavior. Fine.

### 492. `max_cpu_time(0)` may kill immediately

Fine if user granted. Tests avoid.

### 493. `max_address_space` too tight may make sandbox-exec fail

Lockin docs note. Fine.

### 494. `max_processes` on Linux may affect threads? RLIMIT_NPROC counts processes/threads per user; can be tricky. Not m2.

### 495. The scope should mention `disable_core_dumps` after other rlimits order irrelevant

Fine.

### 496. `NetworkPlan::AllowAll` should not inject proxy env

If implementing proxy env injection only for Proxy. Add test maybe.

### 497. Parent env pass with non-Unicode values

SP3 uses `std::env::var_os`; `SandboxedCommand::env` accepts OsStr. But `plan.env.set` values are Strings. Good. Tests likely Unicode. Fine.

### 498. Env pass keys are Strings from compile; should validate no `=`/NUL? m1 likely TOML strings. OS env keys cannot contain `=`/NUL. If parent has weird key impossible. Fine.

### 499. `RFL_BUS_FD_NUMBER` as `3` should be a public const for fixture/tests

Good to define.

### 500. Fixture parses `RFL_BUS_FD` as u32 but raw fd is i32

Use `RawFd`/`i32`; reject negative. Env value "3" fine.

### 501. `RFL_PLUGIN` canonical id useful for logging; good

Yes.

### 502. Potential issue with inherited fd and lockin force_cloexec

Lockin `map_fd` should preserve explicitly inherited fd. Scope cites. Good.

### 503. `CLOEXEC defaults irrelevant` wording should be replaced with explicit ownership/flags

Covered.

### 504. `socketpair(AF_UNIX, SOCK_STREAM, 0)` protocol arg type in nix may be `None`

Implementation detail. Scope can be high-level.

### 505. Need to close child fd in parent after spawn? lockin owns/moves it

Covered.

### 506. Need to close core fd after serve loop ends

Transport drop. Fine.

### 507. `Broker` storing PeerHandle means outbound channel remains alive even after serve task abort? PeerHandle holds tx; serve owns rx. If serve aborts, rx drops, notify returns closed. Good.

### 508. Registered guard drop should happen before dropping broker? Fine.

### 509. `Broker` clone held by service can create reference cycle? Service holds Broker; Broker holds PeerHandle; PeerHandle does not hold service. No cycle.

### 510. `SpawnedPlugin` holds peer and registered guard; guard unregisters broker. Good.

### 511. `PluginSupervisor` holds Arc/clone Broker and spawned plugins. Fine.

### 512. Test harness fake lock builder must grant observer subscribe patterns but not publish unless ack calls use RPC not bus publish

Fine.

### 513. If observer acks via peer.call to core, no publish grant needed

Yes.

### 514. If observer acks via bus.publish, needs publish grant and broker route back. Simpler to use peer.call.

Good.

### 515. Core service injection for test acks is recurring missing API

Covered.

### 516. Production supervisor service should be extensible for future m4 methods

Maybe design now with service router composition:

- bus.publish handler;
- optional extra service for tests/future.

Scope currently hardcodes only bus.publish.

### 517. `Service` handles both requests and notifications, so production bus service can route unknown core.fixture in tests only if injected

Covered.

### 518. A service router module may belong in m2

Could be internal.

### 519. `Broker::handle_plugin_publish` should not be async if fan-out notify is sync

`PeerHandle::notify` is sync. Good. If lifecycle rejection uses publish_core sync. Fine.

### 520. `PeerHandle::call` tests async

Good.

### 521. `publish_core` sync can enqueue notifications but not await delivery

Tests receiving event must wait on observer. Good.

### 522. Fan-out order over BTreeMap deterministic if registry uses BTreeMap

Use BTreeMap keyed by CanonicalId for deterministic tests. Scope does not say. Nice to add.

### 523. `CanonicalId` ordering exists? It is BTreeMap key in m1, yes.

Fine.

### 524. Broker registry duplicate check with BTreeMap easy

Good.

### 525. If fan-out excludes publisher by canonical, duplicate canonical impossible

Good.

### 526. If publisher not registered but canonical in ACL, exclusion irrelevant

Return NotRegistered.

### 527. `Broker::publish_core` should use publisher identity core in event if event schema includes publisher

Define `PublisherIdentity` enum or string. Tests may assert core publisher. If only plugin canonical needed, include optional.

### 528. `BusEvent` publisher field could leak core as `"core"`

Fine.

### 529. Security RFC says broker tags every event with publisher plugin id, not necessarily payload field

Could be internal metadata only. But tests want it. Decide.

### 530. Fittings `Request.metadata` skip serialize could carry publisher internally? Not for bus.event notifications. Simpler payload field.

### 531. Plugin authors may not need publisher field because topic namespace identifies plugin id for plugin topics

But canonical id differs from topic id. Tests want canonical. Up to design.

### 532. `core.lifecycle.publish_rejected` payload canonical string gives canonical anyway

Fine.

### 533. `BusEvent` should maybe include `publisher_canonical` only for plugin publishers

Could be `publisher: { kind, canonical? }`. Avoid overdesign.

### 534. Scope should align with Stream A schema ownership if adding BusEvent fields

Acceptance drift item for bus.event method name should include schema drift.

### 535. m2 says Stream A owns payload schemas but then defines PublishMsg/TaintEntry; acceptable as milestone API but retrospective should log.

### 536. `Payload` in PublishMsg is bus event payload; plugin can publish any schema. Later schema validation per topic maybe m4/m5.

Fine.

### 537. `provider.*` rejection events in m2 may surprise provider plugin installed by default in m5 only, so okay.

### 538. `rfl-openai` not in m2

Good.

### 539. `outpost-proxy` direct dependency in rafaello-core might pull Tokio features

Fine.

### 540. `outpost_proxy::ProxyHandle` drop shuts proxy down; good

Scope uses. Good.

### 541. `ProxyStart` test could use handle.listen_addr if SpawnedPlugin exposes proxy info under cfg(test)

Need design.

### 542. Better proxy integration test: fixture in proxy mode dumps `HTTP_PROXY` env and tries CONNECT to local allowed host

This would exercise env + lockin + outpost. Scope currently defers real CONNECT, but then proxy support is barely tested. Consider adding.

### 543. If no real network in tests, do not claim proxy fully wired

Covered.

### 544. `NetworkPolicy::from_allowed_hosts(["example.com"])` with no CONNECT is only a construction test

Yes.

### 545. `outpost_proxy::start` binds ephemeral port; no port bind failure likely

`ProxyStart` hard to test without injection. Fine.

### 546. `ProxyStart` error variant not covered by tests

Optional; hard to force.

### 547. `Socketpair` failure hard to test

Fine.

### 548. `Lockin` missing syd error hard to test in devshell

Fine.

### 549. `Spawn` std::io error test should be real if scoped

Current drop_during_spawn uses false. Fix.

### 550. `EntryNotExecutable` may cover common spawn error instead

Okay.

### 551. The scope should not overpromise every failure step negative-tested if hard to force without injection

It says negative matrix exercises unwinding. Be realistic.

### 552. Use dependency injection for process/proxy/fittings builders if serious about unwinding tests

Could be overkill for m2, but otherwise drop exact unwinding test claims.

### 553. `manual-validation.md` could capture fd leak instead of automated fd-count

Better.

### 554. Code-review should check no bypass path

Good.

### 555. Commit count estimate 18-25 seems plausible but maybe high

Fine.

## Required rewrite before round 2

At minimum, revise the following before asking for another ratification pass:

1. **Fix W1/W2 dependency coordinates** against the actual repository:
   - fittings paths,
   - lockin package name,
   - transport dependency,
   - async-trait/dev test deps,
   - nix features if using signals.

2. **Replace the fixture binary strategy.**
   - Do not rely on `env!("CARGO_BIN_EXE_rfl-bus-fixture")` from `rafaello-core` tests unless proven.
   - Decide whether fixture is a `rafaello-core` bin target, an explicitly built workspace binary, or copied into temp plugin dirs.
   - Address m1 compile/digest/entry-inside-plugin-dir constraints.

3. **Rewrite SP3/F3 around the actual fittings API.**
   - Core side: likely one `Server` with `PeerHandle`.
   - Plugin side: likely one `Client` with `with_service` and notification handler.
   - Define inherited-fd transport concretely.

4. **Rewrite B3 namespace enforcement from string-prefix checks to structural parsing.**
   - Reject unknown top-level namespaces.
   - Reject/handle short plugin topics.
   - Remove `auto_subscribes` from publish authority unless explicitly decided otherwise.
   - Add tests for the missing cases.

5. **Define `BusEvent` and publisher identity.**
   - Include whatever fields tests assert.
   - Reconcile with overview §4.5 `request_id`.
   - Define lifecycle rejection event schema.

6. **Fix broker error semantics.**
   - Add `NotRegistered` or equivalent.
   - Add wrong-arity `in_reply_to` error or broaden naming.
   - Add unknown namespace error or rename reserved variant.
   - Decide where `InvalidPayload` decoding happens.

7. **Clarify direct fan-out vs core canonical re-emission.**
   - Do not accidentally deliver `plugin.<id>.tool_result` directly contrary to security RFC §5.4.1.
   - If m2 intentionally defers that, state the staged behavior and risk clearly.

8. **Redesign supervisor public API.**
   - `async fn spawn(...)` with a return type that supports multi-spawn tests.
   - Add wait/exit-status access if tests assert child exit.
   - Add service injection or test hook if plugin-to-core calls and observer acks are in scope.
   - Resolve `Broker` vs `Arc<Broker>` double-Arc shape.

9. **Rewrite `SpawnError` against real APIs.**
   - `lockin`/`anyhow::Error`, not `lockin_sandbox::Error`.
   - `FittingsError`, not `fittings_core::Error`.
   - Add missing `Internal`/duplicate-registration/lifecycle variants or separate shutdown error.

10. **Fix lifecycle semantics.**
    - Separate synchronous Drop best-effort from async graceful shutdown.
    - Define SIGTERM/SIGKILL implementation using real APIs.
    - Reap children.
    - Replace `kill -0` zombie-prone tests.
    - Add configurable shutdown grace API if tests need short grace.

11. **Add proxy env injection.**
    - Mirror lockin CLI env behavior for proxy mode.
    - Test env injection and preferably a real CONNECT path, or weaken proxy claims.

12. **Fix env ordering and reserved checks.**
    - Reserved env checks before resource allocation.
    - Reserved vars injected last or otherwise impossible to override.
    - Include `RFL_HELPER_FD` truthfully: m1 does not currently reject it.

13. **Fix private-state/current-dir story.**
    - Decide whether supervisor sets cwd.
    - Decide how fixture knows private-state path.
    - Ensure private-state directory exists before write test.

14. **Make the fixture protocol complete.**
    - Add dump_env/write_private_state/report_open_result/observer ack behavior.
    - Define readiness/hold-open semantics for service modes.
    - Add notification handling for `bus.event`.

15. **Fix the test matrix.**
    - Replace invalid `**` observer pattern.
    - Add unknown namespace, short plugin topic, wrong arity, duplicate registration, unsubscribed drop, proxy env, reserved env pass cases.
    - Remove or redesign non-executable/flaky fd-count/process-table assertions.
    - Choose env serialization dependency/strategy now.

16. **Correct manual validation.**
    - Do not assert exactly fds 0/1/2/3.
    - Add tracing subscriber if expecting `RUST_LOG` output.
    - Use manifest-path-qualified cargo commands.
    - Specify paths for `manual-validation.md` and `retrospective.md`.

17. **Weaken or enforce the "corresponds to a lock entry" claim.**
    - Current public `CompiledPlugin`/`BrokerAcl` structs are forgeable.
    - Either add validation/opaque plan types or stop claiming supervisor can prove lock correspondence.

18. **Add retrospective drift items.**
    - Unknown namespace validation gap in m1/m2.
    - `request_id` omission/staging.
    - Provider namespace staged rejection despite m1 provider ACL fields.
    - `core.lifecycle.publish_rejected` schema.
    - `bus.event` method/schema.

Until these are addressed, `commits.md` should not start.
