# m2-broker-spawn commits.md — pi review round 1

> Review target: `rafaello/plans/milestones/m2-broker-spawn/commits.md`
> against ratified `scope.md` round 11.
>
> Verdict: **not ready to ratify**. The plan is substantially on the right
> track, but several per-commit greenness traps and acceptance-coverage gaps
> would surface during Phase 3 agent work. Fixes are mostly plan edits rather
> than scope changes.

## Executive summary

The ordered commit list has a good shape: workspace wiring, error/wire types,
broker registration, broker publish enforcement, supervisor Phase A, supervisor
Phase B, fixture/harness, lifecycle, and demo-bar tests. It also carries many
hard-won details from the ratified scope: `--features test-fixture`,
`lockin::tokio_command`, `PeerHandle::notify(Value)`, post-spawn reap on
unwind, `SpawnPaths`, `RFL_TOPIC_ID`, no boot auto-emit, and the
result-routing suppression.

However, round 1 is **not executable as written**. The most important issues
are:

1. c06 asks tests to round-trip types that are intentionally one-way serde
   types in scope.
2. supervisor resource ownership is underspecified in a way that can let
   external `SpawnHandle` clones keep `RegisteredPlugin` / `ProxyHandle`
   resources alive after supervisor shutdown/drop.
3. c20 depends on fixture behavior scheduled for c21.
4. several named scope acceptance behaviors are not scheduled in any commit.
5. c15's duplicate-canonical test is contradictory and not implementable
   without an explicit test hook.
6. c23's harness section uses stale/wrong m1 API names for lock validation and
   lock fields.
7. c24 shutdown does not explicitly wait for the reaper after forced SIGKILL.

None of these require reopening `scope.md`; they require a round-2 rewrite of
`commits.md`.

## Blocking findings

### B1. c06 acceptance cannot compile as written

**Where:** `commits.md` c06, especially the wire type definitions and
acceptance test.

**Problem:** c06 defines:

- `PublishMsg` as `Deserialize` only.
- `BusEvent` and `PublisherIdentity` as `Serialize` only.

The acceptance asks `tests/bus_types_round_trip.rs` to serialize and deserialize
both `PublishMsg` and `BusEvent`, asserting byte equality. That cannot compile
unless the implementation drifts away from scope by adding derives that were not
specified.

**Why it matters:** this is an immediate per-commit greenness failure in c06.
Agents will either fail the tests or silently broaden the public wire contract.

**Fix:** rewrite c06 acceptance to match the one-way contract:

- For `PublishMsg`: decode `serde_json::Value -> PublishMsg`, assert fields,
  and assert `deny_unknown_fields` rejects unknown keys.
- For `BusEvent`: encode `BusEvent -> serde_json::Value`, inspect fields, or
  deserialize into a test-only permissive struct.
- Optionally rename the test from `bus_types_round_trip.rs` to something like
  `bus_wire_types_schema.rs` unless round-trip derives are intentionally added
  to scope.

### B2. Supervisor shared-state/resource ownership can violate the ratified lifecycle rule

**Where:** c14/c20 define `SpawnHandle(Arc<SpawnedState>)` and supervisor
registry insertion; c24/c25 later claim shutdown/drop releases broker
registration and proxy resources.

**Problem:** c14 sketches:

```text
SpawnHandle(Arc<SpawnedState>)
spawned: Mutex<BTreeMap<CanonicalId, Arc<SpawnedState>>>
```

c20 inserts the same `Arc<SpawnedState>` into the supervisor registry and
returns a cloned handle. c24/c25 then say shutdown/drop drops the
`RegisteredPlugin` guard and `ProxyHandle`. If those fields live directly inside
`SpawnedState`, external `SpawnHandle` clones can keep them alive after the
supervisor drains its registry. That contradicts scope §SP1's rule that **the
supervisor owns child lifetime, not external clones** and that external handles
can only observe cached `ReaperOutcome` after teardown.

**Why it matters:** this is a subtle resource-lifecycle bug. It could leave the
broker registration live, keep fan-out enabled, keep the proxy alive, or delay
cleanup until every test clone drops.

**Fix:** specify the `SpawnedState` layout before implementation. For example:

- Split handle-observable state from supervisor-owned resources:
  - `SpawnHandle` holds `Arc<SpawnObservation>` containing canonical, topic_id,
    cached pid, peer clone, watch receiver, maybe closed/cached outcome.
  - supervisor registry holds `ManagedSpawn { observation: Arc<...>,
    registered: Option<RegisteredPlugin>, proxy: Option<ProxyHandle>,
    serve_join, watcher_join, ... }`.
- Or keep a single `Arc<SpawnedState>` but put supervisor-owned resources behind
  `parking_lot::Mutex<Option<_>>`, and require shutdown/drop to `take()` them
  exactly once. External handles must not keep these resources alive.

The plan should state this explicitly in c14/c20/c24/c25 so per-commit agents do
not invent incompatible shapes.

### B3. c20 depends on fixture behavior that only lands in c21

**Where:** c20 acceptance asks for `tests/supervisor_spawn_fixture_round_trip.rs`
to spawn the c03 fixture in `respond_peer_call` mode, but c21 is the commit that
replaces c03's trivial `main` with real fixture init and implements
`respond_peer_call`.

**Problem:** c20 has no real fixture mode available. The text allows either a
transient sleeper binary or an ignored test. The ignored-test option is not a
valid per-commit greenness proof: c20 can pass without proving the intended
behavior, and c21 might forget to unignore it.

**Fix options:**

1. **Preferred structural reorder:** split fixture work earlier.
   - c03: target scaffold only.
   - before supervisor Phase B completion, add a minimal fixture commit that
     implements only `RFL_BUS_FD` transport + a hold-open/sleeper or
     `respond_peer_call` mode.
   - c20 can then use the real fixture.
2. Or make c20's temporary sleeper mandatory, not optional, and require c21
   acceptance to remove the temporary binary/test path and unignore/replace the
   c20 test.

Do not leave the plan with an optional `#[ignore]` path.

### B4. Required scope acceptance items are not scheduled

**Where:** scope acceptance summary says every named behavior in the positive
and negative matrices must be implemented. Several named behaviors have no
commit home in `commits.md`.

Missing or under-scheduled items:

- `bus_pattern_matches.rs` from scope positive matrix. No commit schedules it.
  Add it to c06 or c07.
- `broker_unsubscribed_plugin_does_not_receive.rs` from scope negative matrix.
  Add it to c12 fan-out acceptance.
- `supervisor_env_pass_set_applied.rs`,
  `supervisor_env_set_overrides_pass.rs`, and
  `supervisor_env_clear_strips_unrelated.rs`. c18 says env behavior is
  unobservable and later references only one env test vaguely; c26 covers proxy
  env, not pass/set/clear semantics. Add a dedicated post-fixture env-test
  commit or include all three in c26/c28.
- `supervisor_peer_call_core_to_plugin.rs` and
  `supervisor_peer_call_plugin_to_core.rs` are partially represented by fixture
  tests, but the canonical scope test names/behaviors should be explicitly
  scheduled after c21/c23.
- `supervisor_spawn_fixture_happy_path.rs` from scope is replaced by
  `supervisor_spawn_fixture_round_trip.rs` in c20 and later
  `supervisor_bus_publish_round_trip_two_plugins.rs` in c28. Pick a canonical
  name and ensure the exact headline publish/observer behavior is scheduled.
- Real `supervisor_spawn_duplicate_canonical_refused.rs` after a successful
  spawn. c15's synthetic in-flight check is not the scope behavior.
- `retrospective.md` is required by scope acceptance but no final commit writes
  it.

**Fix:** add explicit acceptance bullets/commit rows for each missing behavior,
or state which scheduled test covers the named behavior. For docs, add a final
commit after manual validation that writes `retrospective.md`, or expand c29 to
write both `manual-validation.md` and `retrospective.md`.

### B5. c15 duplicate-canonical test is internally contradictory

**Where:** c15 acceptance for `supervisor_spawn_duplicate_canonical_refused.rs`.

**Problem:** it says “spawn A successfully” and then immediately notes that c15
cannot complete a successful spawn because Phase B lands later. It proposes a
synthetic “in_flight already contains canonical” precondition, but no test hook
for inserting into the private `in_flight` set is planned.

**Why it matters:** an agent cannot implement this test honestly in c15 without
adding an unplanned test-only mutator or reaching into private fields.

**Fix:** split the checks:

- c15: test only the synthetic in-flight path via an explicit `#[cfg(any(test,
  feature = "test-fixture"))]` hook such as
  `test_hooks().insert_in_flight_for_test(canonical)` if that is acceptable; or
  do not test duplicate spawn yet.
- c20/c24: after real spawn works, add the real duplicate-spawn test required by
  scope, asserting no socketpair/proxy/child deltas on the second spawn.

### B6. c23 harness text uses stale/wrong m1 API names and validation shape

**Where:** c23 `FixtureLockBuilder` description.

**Problem:** the plan says it constructs a `Lock` with
`granted_capabilities`, `bindings`, and digest fields, then runs
`validate::lock(&lock, &path_context)`. Current m1 code uses:

- `PluginEntry { grant: Grant, bindings, flags, ... }`, not
  `granted_capabilities`.
- `validate::lock(&Lock, &LockValidationContext)`, not `PathContext`.
- `LockValidationContext` requires `project_root`, `home`, `plugin_dirs`,
  `cache_root`, and `state_root`.

**Why it matters:** c23 is the commit that most later supervisor tests depend
on. If the harness prompt contains stale API names, the agent will waste time or
invent wrappers.

**Fix:** rewrite c23 to name the current m1 API exactly:

- Construct `PluginEntry { entry, digest, manifest_digest, granted_at, grant,
  bindings, flags }`.
- Build `LockValidationContext { project_root, home, plugin_dirs, cache_root,
  state_root }`.
- Call `validate::lock(&lock, &lock_validation_context)`.
- Then call `compile::compile_plugin(&lock, canonical, &PathContext { ... },
  &RecomputedDigests { ... })` for each plugin and `broker_acl::compile(&lock)`.

### B7. c24 shutdown does not explicitly reap after forced SIGKILL

**Where:** c24 shutdown algorithm.

**Problem:** the algorithm waits for graceful exit until the grace timeout,
sends SIGKILL on timeout, then proceeds to drop proxy/abort serve-loop/build the
report. It does not explicitly wait for the reaper/watch transition after the
forced kill.

**Why it matters:** scope §SP1 says `shutdown(self).await` is the deterministic
cooperative path and blocks on the reaper. Returning before the forced child is
reaped risks zombies, racy `ShutdownReport`s, and flaky tests.

**Fix:** after SIGKILL, await the same watch/reaper outcome before returning the
per-plugin result. If a bounded fallback is needed, state it explicitly and
record a `ShutdownFailure`, but do not imply the child is fully shut down before
the reaper has observed it.

## Per-group and per-commit findings

### Group 0 — workspace deps + fixture scaffold

- c01/c02 generally match scope §W1/§W2.
- c03 is fine as a target scaffold, but later c20 assumes behavior that c03 does
  not provide. Either move minimal fixture behavior earlier or avoid using the
  c03 binary for behavioral tests before c21.
- c03 `fixture_binary_resolves.rs` is useful, but ensure it is only compiled
  when `test-fixture` is enabled. The plan already says this.

### Group 1 — reserved env list extension

- c04 is a good small back-reach to m1's scrubber.
- The acceptance should table-test all new reserved names, not leave “or add a
  sibling” to driver choice. Per-commit prompts are supposed to be verbatim and
  deterministic.
- The text says “m1 c04 already rejects” in c16; in m1, the scrubber landed as
  m1 c21. Use neutral wording like “the m1 scrubber now rejects after m2 c04”.

### Group 2 — bus error surface + wire types

- c05 error-surface compile tests are useful. Watch out for constructing source
  variants that need real `anyhow::Error`, `std::io::Error`, `FittingsError`,
  and `outpost::DomainPatternParseError`; build-only tests should avoid brittle
  parsing hacks where possible.
- c06 has the blocking serde-direction issue above.
- Add `bus_pattern_matches.rs` here or in c07. This is a pure test and should be
  cheap.

### Group 3 — broker registration + lifecycle

- c07 is a sensible registration slice.
- c07's fake `PeerHandle` strategy must be made concrete. Current fittings has
  `PeerHandle::new(mpsc::Sender<OutboundNotification>, DroppedNotifications,
  CancellationToken)`, so a test peer kit can use that. Spell this out instead
  of leaving it to the agent.
- c08 boot event path is well placed and correctly explicit (no auto-emit).

### Group 4 — broker publish path

- c09/c10/c11 split grammar/namespace/grants/arity cleanly.
- c12 should add `broker_unsubscribed_plugin_does_not_receive.rs`.
- c13 rejection events are well placed. Ensure every rejection path emits the
  lifecycle event after extracting best-effort topic for invalid payload.
- c13 `publish_core` should validate grammar before namespace if preserving the
  scope's “grammar before namespace” rule for core too; make ordering explicit
  if needed.

### Group 5 — supervisor Phase A

- c14 scaffold is useful but should define the resource ownership shape now (see
  B2), otherwise later lifecycle commits will fight the initial type layout.
- c14 `peer() -> &PeerHandle` “panics until c19” requires a placeholder
  `PeerHandle` value. Prefer making `SpawnHandle` impossible to construct until
  real spawn, or use a detached test peer explicitly; avoid a public method that
  panics in normal type-compile tests.
- c15 duplicate test must be fixed/deferred (see B5).
- c16 implements invalid proxy allow-host dry-run but does not test it. Add a
  `supervisor_spawn_invalid_proxy_allow_hosts_refused.rs` or equivalent.

### Group 6 — supervisor Phase B

- c17 resource allocation/unwind smoke tests are useful.
- c17/c18 rely on the fixture binary path for an executable entry but do not
  spawn yet; that is okay as long as all tests run with `--features
  test-fixture`.
- c18 says env behavior is unobservable and then references later tests that are
  not actually scheduled. Add an explicit env-test commit after fixture/harness.
- c19 service composition should name the real fittings types:
  `fittings_core::service::Service`, `fittings_core::message::{Request,
  Response, JsonRpcId}`, and `fittings_core::context::ServiceContext`.
- c19 says “notification handler calls broker and returns Ok so transport stays
  alive”. In fittings, notifications are requests with `id = None`; returning a
  `Response` is swallowed by the server. The plan should still require a valid
  `Response { id: JsonRpcId::Null, result: Value::Null, ... }` from the service
  call.
- c19 rollback assertion is self-contradictory. It should assert
  `contains_plugin == true` for ACL membership and
  `try_reserve_registration == Ok(())` for no live registration.
- c20 `child_pid()` cannot query the child after the reaper task owns it. Cache
  `pid: Option<u32>` before moving the child into the reaper and return `None`
  once the watch outcome is `Some`.
- c20 fixture dependency must be structurally fixed (see B3).

### Group 7 — fixture binary + test harness

- c21/c22 capture the right fixture-mode model, including readiness and flush
  ack.
- c21 should include the exact return/error convention for unknown
  `RFL_FIXTURE_MODE` (exit code and stderr). Otherwise malformed env tests may
  hang.
- c22 `respond_peer_call` extensions include `dump_env`, write/read probes,
  etc. Good.
- c22 `call_core_then_exit` says harness returns `{"n": 43}`; the fixture only
  needs a successful response, but the expected response should be specified if
  it is asserted.
- c23 harness has stale m1 API names (see B6).
- c23 introduces `Vec<ExtraService>` but no concrete type alias/trait shape for
  test extra services. It should align with scope §SP2's `ExtraServiceFactory =
  Arc<dyn Fn(CanonicalId) -> Box<dyn Service + Send + Sync> + Send + Sync>` or
  explicitly define a test-side wrapper.

### Group 8 — lifecycle

- c24 needs forced-kill reaper wait (see B7).
- c24 adding `RFL_FIXTURE_TRAP_SIGTERM=1` is reasonable, but it modifies fixture
  behavior after c21/c22. Mention that c24 updates fixture tests if necessary.
- c25 Drop logic should use the resource-ownership shape from B2 so external
  handles cannot keep `RegisteredPlugin`/proxy alive.
- c25 acceptance is valuable: held `SpawnHandle` clone observes the cached
  outcome after supervisor drop.

### Group 9 — proxy + lockin denial proofs

- c26 is a good no-new-code integration-test commit, but only covers proxy env.
  Do not treat it as covering pass/set/env_clear tests unless those are added.
- c27 denial proofs match scope's platform-agnostic intent. Keep errno matching
  broad (`EPERM` / `EACCES` / `ENOENT`).

### Group 10 — cross-plugin scenarios + manual validation

- c28 covers important demo-bar behavior. It should also be a home for missing
  env tests if no separate commit is added.
- c29 manual validation is needed, but it should not instruct a per-commit agent
  to push `rafaello-v0.1` to origin. The driver should own pushing and CI URL
  capture.
- c29 uses `--test supervisor_spawn_fixture_round_trip`, while scope manual
  validation references `supervisor_spawn_fixture_happy_path`. Pick one
  canonical test file name.
- c29 should also write `retrospective.md` or be followed by c30 docs commit for
  retrospective, because scope acceptance requires it.

## Structural reorderings recommended for round 2

1. **Move minimal fixture behavior before supervisor Phase B completion.**
   - Keep c03 as target scaffold if desired.
   - Add an early fixture-minimal commit before c20 that implements
     `RFL_BUS_FD` setup and either `respond_peer_call` or a hold-open mode.
   - Then c20 can prove real spawn/reaper/serve-loop behavior without ignored
     tests or temporary binaries.

2. **Define supervisor resource ownership in the scaffold commit.**
   - c14 should specify the observation-vs-managed-resource split or
     `Option::take` model.
   - Later lifecycle commits should not need to redesign `SpawnedState`.

3. **Move real duplicate-spawn testing after real spawn exists.**
   - c15 can test only cheap validation and maybe an explicit in-flight hook.
   - A later commit after c20/c23 should run the actual “spawn A, spawn A again”
     test required by scope.

4. **Add a dedicated env behavior test commit after c23.**
   - Tests: pass+set applied, set overrides pass, env_clear strips unrelated.
   - This avoids pretending c18 can test env application before fixture services
     exist.

5. **End with documentation closeout that writes both manual validation and
   retrospective.**
   - Either expand c29 or add c30.
   - Driver supplies CI URL/push evidence; per-commit agent should not push.

## Non-executable or ambiguous acceptance text to rewrite

- **c06:** round-trip serde test incompatible with one-way derives.
- **c07:** “pick fake PeerHandle strategy at implementation time” is too loose;
  specify the test peer kit.
- **c15:** “spawn A successfully” despite no Phase B; replace with a clear
  synthetic in-flight hook or defer.
- **c16:** “Acceptance. Four new tests” lists five tests and omits invalid
  allow-hosts.
- **c18:** “No new tests” but references later tests that are not scheduled;
  add a concrete later commit.
- **c19:** rollback assertion says `try_reserve_registration` returns
  `NotInAcl — actually Ok(())`; remove the contradiction.
- **c20:** optional `#[ignore]` path is not acceptable for a headline test.
- **c20:** `child_pid()` wording implies reading from a child value moved to the
  reaper; specify cached pid.
- **c21/c22:** make unknown fixture mode behavior explicit.
- **c23:** stale m1 API names (`granted_capabilities`, `path_context`) need
  correction.
- **c24:** add wait-after-SIGKILL in shutdown algorithm.
- **c29:** do not require the implementation agent to push; driver owns CI
  capture. Also align test file name with scope.

## Things the draft gets right

- Correctly keeps `rfl-bus-fixture` inside `rafaello-core` behind
  `required-features = ["test-fixture"]`, so `CARGO_BIN_EXE_rfl-bus-fixture`
  resolves for integration tests.
- Correctly repeats the need for `--features test-fixture` in per-commit cargo
  invocations.
- Correctly avoids `fittings-wire` as a direct rafaello-core dependency and uses
  fittings-core/server/client/transport paths.
- Correctly uses lockin's Tokio API (`SandboxBuilder::tokio_command`) rather
  than sync `command`.
- Correctly models `SpawnPaths` as caller-supplied rather than inferred from
  write grants.
- Correctly stages broker enforcement: decode/grammar, structural namespace,
  own-topic grant, in-reply-to arity, fan-out, then rejection events.
- Correctly distinguishes `UnknownNamespace` from `PublishOnReservedNamespace`.
- Correctly excludes publisher from its own fan-out.
- Correctly suppresses direct fan-out for `tool_result` / `rpc_reply` pending
  m4 re-emission.
- Correctly makes boot emission explicit via `publish_boot()` and does not call
  it from `Broker::new` or `PluginSupervisor::new`.
- Correctly uses readiness + start handshakes for fixture determinism.
- Correctly documents `Client::notify` vs `PeerHandle::notify` FIFO behavior for
  publish-and-flush fixture modes.
- Correctly preserves the no-TMPDIR ABI caveat after `env_clear`.
- Correctly includes both upper- and lower-case proxy env vars and clears
  `NO_PROXY`/`no_proxy`.
- Correctly plans test-only supervisor hooks as per-supervisor rather than
  global atomics.
- Correctly uses best-effort `tracing::warn!` for failed fan-out and does not
  fail the publish call on subscriber transport errors.
- Correctly includes post-spawn kill-and-reap unwind requirements for transport
  setup / registration failures.

## Round-2 checklist

Before asking for pi review round 2, update `commits.md` to:

- [ ] Fix c06 serde acceptance.
- [ ] Specify supervisor managed-resource ownership separate from external
      `SpawnHandle` clones.
- [ ] Reorder or split minimal fixture work so c20 can run real tests.
- [ ] Add missing scope tests/behaviors: `bus_pattern_matches`, unsubscribed
      broker fan-out negative, three env tests, real duplicate spawn,
      peer-call canonical tests, and retrospective.
- [ ] Correct c15 duplicate test wording/hook.
- [ ] Correct c23 harness API names to current m1 code.
- [ ] Add forced-kill reaper wait to c24 shutdown.
- [ ] Remove c19 contradictory rollback assertion.
- [ ] Remove per-commit-agent branch push requirement from c29.
- [ ] Align canonical test names between scope/manual-validation/commits.
