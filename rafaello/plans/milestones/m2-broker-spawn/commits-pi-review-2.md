# m2-broker-spawn commits.md — pi review round 2

> Review target: `rafaello/plans/milestones/m2-broker-spawn/commits.md`
> at round-2 draft commit `6e01acc`, reviewed against ratified
> `scope.md` round 11 and the prior review
> `commits-pi-review-1.md`.
>
> Verdict: **not ready to ratify**. Round 2 fixed the largest
> round-1 structural problems, but several per-commit greenness
> traps remain and the rewrite introduced new API/ordering
> contradictions. These are plan edits, not scope reopeners.

## Executive summary

Round 2 is meaningfully better than round 1. The wire-type serde
contract now matches scope, the fixture work is moved before the
first supervisor happy-path test, duplicate-spawn testing is no
longer synthetic, the supervisor state model now explicitly splits
handle-observable state from supervisor-owned resources, and the
forced-kill shutdown path now waits for the reaper observation.

However, the plan is still not executable as written. The biggest
remaining risks are immediate compile failures in c19/c23, an
impossible reaper-join ownership design in c21/c25, an async mutex
hold in c25, a c20 fixture rewrite that would break c03 tests, and
a headline test that still does not cover the scoped demo-bar
behaviour. Fix these before owner ratification.

## Round-1 closure check

### B1. c06 one-way serde contract

**Status: closed.** c06 no longer asks for `PublishMsg` /
`BusEvent` round-trips. It now tests `PublishMsg` as decode-only,
`BusEvent` as encode-only, and uses a permissive test-only receive
struct for schema inspection. The renamed
`bus_wire_types_schema.rs` matches the scoped one-way derives.

### B2. Supervisor shared-state/resource ownership

**Status: mostly closed, with a new ownership bug.** c14 now defines
`ManagedSpawn` as supervisor-owned state and `SpawnObservation` as
handle-shared state. That addresses the original concern that
external `SpawnHandle` clones could keep `RegisteredPlugin` /
`ProxyHandle` alive. New blocker B2 below covers a different
ownership issue: the c21 watcher cannot both await and store the same
Tokio `JoinHandle`.

### B3. c20 depending on fixture behaviour scheduled later

**Status: structurally closed, with a new fixture-ordering bug.** The
minimal fixture now lands before the first real supervisor happy-path
commit. There are no ignored tests. New blocker B4 below covers the
new problem introduced by c20's universal fd parsing order.

### B4. Missing scope acceptance items

**Status: partially closed.** The round-2 plan added explicit homes
for several missing behaviours: `bus_pattern_matches.rs`,
`broker_unsubscribed_plugin_does_not_receive.rs`, invalid proxy
allow-hosts, the three env tests, both peer-call directions, and the
real duplicate-spawn test. But the canonical headline
`supervisor_spawn_fixture_happy_path.rs` still does not exercise the
scope's publish/observer demo-bar behaviour. See new blocker B5.

### B5. c15 duplicate-canonical contradiction

**Status: closed.** c15 no longer tries to test a synthetic in-flight
precondition. The real duplicate-spawn test is deferred until c24,
after real spawn + harness work exists.

### B6. c23 stale m1 harness API names

**Status: partially closed.** c23 now names the right validation /
compile functions and the right `LockValidationContext` shape. But it
still names two wrong concrete types (`PathBuf` for `PluginEntry.entry`
and `PluginFlags` instead of `LockFlags`). See new blocker B6.

### B7. c24/c25 shutdown must reap after forced SIGKILL

**Status: closed in intent.** c25 now explicitly continues waiting on
the watch after SIGKILL so `shutdown(self).await` does not return
before the reaper observes the child. New blocker B3 covers the
implementation sequencing problem around draining `managed` under a
synchronous mutex.

## Blocking findings

### B1. c19 still uses a non-existent fittings `Response` shape

**Where:** c19, bus.publish service implementation.

**Problem:** c19 says the service returns:

```rust
Ok(Response { jsonrpc: ..., id: JsonRpcId::Null, result: Value::Null, error: None })
```

Actual `fittings_core::message::Response` is:

```rust
pub struct Response {
    pub id: JsonRpcId,
    pub result: Value,
    pub metadata: Metadata,
}
```

There is no `jsonrpc` field and no `error` field.

**Why it matters:** c19 cannot compile as written; this is an
immediate per-commit greenness failure.

**Fix:** c19 should require:

```rust
Ok(Response {
    id: JsonRpcId::Null,
    result: Value::Null,
    metadata: Default::default(),
})
```

If unknown methods return `Err(FittingsError::method_not_found(...))`,
the server's existing error mapping will handle the wire error shape.

### B2. c21/c25 reaper watcher design cannot compile

**Where:** c14 `ManagedSpawn`, c21 steps 18-20, c25 shutdown.

**Problem:** c14 stores both:

```rust
reaper_join: Option<tokio::task::JoinHandle<()>>,
watcher_join: Option<tokio::task::JoinHandle<()>>,
```

c21 says a watcher task awaits the reaper `JoinHandle`. A Tokio
`JoinHandle` is consumed by `.await` and is not `Clone`, so it cannot
both be moved into the watcher task and retained in `ManagedSpawn` for
shutdown.

**Why it matters:** agents will hit a borrow/move compile failure or
silently redesign the lifecycle shape in a later commit.

**Fix options:**

1. Store only `watcher_join` in `ManagedSpawn`; the watcher owns the
   reaper handle and publishes `ReaperPanicked` on `JoinError`.
2. Drop the separate watcher task and make the reaper task catch its
   own unwind / publish all outcomes, if that is feasible with the
   desired panic semantics.

Whichever design is chosen, c14/c21/c25 must agree on one concrete
ownership shape.

### B3. c25 implies awaiting while holding `parking_lot::MutexGuard`

**Where:** c25 shutdown algorithm.

**Problem:** c25 describes:

```text
for each `(canonical, mut managed)` in self.managed.lock().drain():
    ... await watch transition ...
    ... await again after SIGKILL ...
```

This holds a synchronous `parking_lot::MutexGuard` across `.await`.

**Why it matters:** this is a common Tokio/clippy greenness trap. It
can make the shutdown future non-`Send`, risks blocking unrelated
supervisor operations, and is contrary to the plan's own emphasis on
snapshot/drain-then-drop patterns elsewhere.

**Fix:** drain under the lock into a local `Vec<(CanonicalId,
ManagedSpawn)>`, drop the guard, then perform async teardown per
plugin.

### B4. c20 breaks c03 `scaffold_only` / unknown-mode tests

**Where:** c20 fixture rewrite.

**Problem:** c03 establishes tests that run the fixture directly with
only `RFL_FIXTURE_MODE=scaffold_only` or `RFL_FIXTURE_MODE=bogus` and
expect exit 0 / 64. c20 says universal fixture init first parses
`RFL_BUS_FD`, then dispatches modes. Without `RFL_BUS_FD`, both of
those direct tests would exit 3 before mode dispatch.

**Why it matters:** c20 regresses earlier green tests and violates the
c03 unknown-mode contract that round 2 explicitly tried to preserve.

**Fix:** dispatch `RFL_FIXTURE_MODE` before fd setup:

- `scaffold_only` exits 0 without requiring `RFL_BUS_FD`.
- unknown modes exit 64 without requiring `RFL_BUS_FD`.
- real bus-backed modes then parse and require `RFL_BUS_FD`.

### B5. Canonical headline test still does not cover scope's demo-bar behaviour

**Where:** c21 `supervisor_spawn_fixture_happy_path.rs`, c30
`supervisor_bus_publish_round_trip_two_plugins.rs`, scope positive
matrix.

**Problem:** scope defines `supervisor_spawn_fixture_happy_path.rs` as
the headline two-fixture publish/observer demo: A publishes on
`plugin.<A>.hello`, B observes the `bus.event`, and the test asserts
topic, payload, and publisher identity.

Round 2's c21 `supervisor_spawn_fixture_happy_path.rs` only spawns a
`respond_peer_call` fixture, checks handle fields, sends SIGTERM, and
waits for an exit. The actual publish/observer behaviour is scheduled
later under a different test name.

**Why it matters:** this leaves the canonical scope acceptance item
unsatisfied even though round 2 claims to have closed the missing-test
finding. It also weakens the milestone's advertised demo bar.

**Fix:** make `supervisor_spawn_fixture_happy_path.rs` cover the
scope's two-plugin publish/observer behaviour. If c21 is too early,
move the canonical-name test to the later commit that has all fixture
modes, and rename the c21 process-lifecycle test to a non-headline
name.

### B6. c23 still names wrong concrete m1 harness types

**Where:** c23 `FixtureLockBuilder` details.

**Problem:** c23 improved the stale API names but still says:

- `PluginEntry.entry` is a `PathBuf`; actual type is `SafePath`.
- `flags: PluginFlags`; actual exported type is `LockFlags`.

Actual m1 code has:

```rust
pub struct PluginEntry {
    pub entry: SafePath,
    pub digest: String,
    pub manifest_digest: String,
    pub granted_at: DateTime<Utc>,
    pub grant: Grant,
    pub bindings: Bindings,
    pub flags: LockFlags,
}
```

**Why it matters:** c23 is the foundation for most later supervisor
integration tests. Wrong concrete field types will send agents down a
compile-failure path.

**Fix:** c23 should explicitly say to use
`SafePath::parse("bin/fixture")?` for `entry`, and `LockFlags` /
`LockFlags::default()` for `flags`.

### B7. c19 post-register reaper test is unobservable as written

**Where:** c19 acceptance,
`tests/supervisor_spawn_post_register_reaps_child.rs`.

**Problem:** c19 still returns an error after the step-17/register
unwind:

```text
spawn returns the "Phase B step 18+" error
```

But the test says `cached_pid` is `Some(_)` immediately after spawn
returns. There is no `SpawnHandle` on `Err`, and `SpawnError` does not
carry the child pid.

**Why it matters:** the test cannot be implemented honestly without an
unplanned hook or changing the error type.

**Fix options:**

- Add a test-only hook recording the last spawned pid / last reaped pid
  for c19 unwind tests, then assert through that hook.
- Or reduce c19 acceptance to observable cleanup facts: registration
  removed, in-flight cleared, fd/proxy cleanup, and no live child if a
  hook can expose the pid.

Do not assert `SpawnHandle` state before any handle is returned.

### B8. c22 `call_core_then_exit` skips the required readiness/start gate

**Where:** c22 fixture mode description.

**Problem:** scope F2 says every real fixture mode performs universal
ready, then waits for `core.fixture.start` before mode-specific work.
Round 2's c22 says `call_core_then_exit` calls
`core.fixture.ping` and exits, without waiting for `start`.

**Why it matters:** this reintroduces a race the two-phase harness was
created to avoid. The fixture could call into the harness before the
harness has completed its deterministic readiness/start sequencing.

**Fix:** specify that `call_core_then_exit` waits for
`core.fixture.start`, then calls `core.fixture.ping`, then exits 0/2.

## Non-blocking findings

### N1. c21 peer-call test should explicitly wait for readiness

`supervisor_peer_call_core_to_plugin.rs` should wait for the fixture's
`core.fixture.ready` handshake before calling `core.fixture.echo`.
Otherwise it can race the fixture's `with_service` installation.
This may already be implicit in the harness, but the c21 text predates
the c23 harness and should say it plainly or defer the test until the
harness exists.

### N2. c17 proxy-port closure assertion may be flaky

c17 says the proxy port is no longer bound immediately after spawn
returns. `outpost_proxy::ProxyHandle` documents that listener closure
is asynchronous after drop and a brief connect-success window is
acceptable. Make the test poll with a bounded timeout or assert only
that the handle was dropped via a hook.

### N3. c30 says “three tests” but lists four

c30's text says “Three integration tests” and then lists four,
including `supervisor_peer_call_plugin_to_core.rs`. It also says the
agent may split at implementation time. For deterministic Phase 3
planning, either split now or make c30 explicitly own all four tests.

### N4. Linux-only gates drift from scope's platform-gating guidance

c17/c19/c20 introduce Linux-only gates for `/proc` and fd-inheritance
style tests. Some are justified by `/proc`, but the c20 fixture direct
socketpair test may not need a permanent Linux-only gate if written with
portable Unix fd inheritance. Non-blocking because scope allows gates
after CI proves a platform issue, but the plan should avoid unnecessary
pre-emptive narrowing.

### N5. c25 SIGTERM-trap wording is too vague

`tokio::signal::ctrl_c` is not a SIGTERM trap. If the fixture needs to
ignore SIGTERM for the forced-shutdown test, c25 should name the actual
Unix signal handling API (`tokio::signal::unix::signal(SignalKind::terminate())`)
or state that the agent must implement a real SIGTERM handler. This is
likely fixable during implementation, but clearer wording avoids a
failed forced-shutdown test.

## Things newly right in round 2

- c06 now aligns with the one-way wire contract instead of widening
  public serde derives.
- c07 explicitly schedules `bus_pattern_matches.rs` and pins a concrete
  `PeerHandle::new`-based test kit strategy.
- c12 now includes the unsubscribed-plugin negative case and result
  routing suppression.
- c13 preserves grammar-before-namespace ordering for `publish_core` and
  routes rejection observability through an internal non-recursive path.
- c14's `ManagedSpawn` / `SpawnObservation` split is the right lifecycle
  direction: external handles observe outcomes but do not own broker
  registration or proxy resources.
- c15 removed the contradictory synthetic duplicate-spawn test.
- c16 now covers all reserved env names in set/pass and includes invalid
  proxy allow-hosts.
- c18 correctly acknowledges that env behaviour is not observable until
  the fixture can dump env, and schedules those tests later.
- c20-before-c21 is the right structural reorder: the first real
  supervisor happy path no longer depends on fixture behaviour that lands
  later.
- c22 correctly uses `Client::notify` rather than `PeerHandle::notify` for
  fixture publish modes, preserving FIFO ordering with the flush call.
- c24 now contains the real duplicate-canonical test after successful
  spawn is available.
- c25 explicitly waits for the reaper after forced SIGKILL, closing the
  zombie/racy-report concern from round 1.
- c26 explicitly relies on the managed-vs-observation split so
  `SpawnHandle` clones cannot keep `RegisteredPlugin` / `ProxyHandle`
  alive.
- c27 gives the three env behaviour tests a dedicated home and uses
  `serial_test::serial(env)`.
- c31 correctly treats branch push and CI URL capture as driver-owned
  actions rather than per-commit agent work.

## Recommended round-3 edit checklist

1. Fix c19 `Response` construction to the actual fittings type.
2. Redesign c14/c21/c25 reaper ownership so a `JoinHandle` is consumed in
   exactly one place.
3. Drain `managed` into locals before any `.await` in c25.
4. Make c20 dispatch `scaffold_only` / unknown modes before requiring
   `RFL_BUS_FD`.
5. Move or rename the canonical `supervisor_spawn_fixture_happy_path.rs`
   so it covers the scoped publish/observer demo.
6. Correct c23 `SafePath` and `LockFlags` type names.
7. Replace c19's unobservable `cached_pid` assertion with a planned
   test hook or observable cleanup assertion.
8. Add the missing `core.fixture.start` wait to `call_core_then_exit`.
