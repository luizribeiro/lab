# pi review 16 — m3 scope round-16 adversarial review

Reviewed: `plans/milestones/m3-tui-sessions/scope.md` at commit `536c750` (3250 lines).

Verdict: **no blockers found**. I would not treat the draft as fully ratifiable as-is because the three high findings can mislead implementation/test design, but all are scope-text/API-alignment fixes rather than architectural blockers. After the high findings below are corrected, the scope should be ratifiable from this review's perspective.

Counts: **0 blocker / 3 high / 2 medium / 2 low**.

## High

### H1. H6 inject-point placement does not match the deleted m2 coverage

`scope.md:2291-2297` defines the H6 "pre-register" injection point as immediately before `broker.register_plugin(...)`. In the current `PluginSupervisor::spawn`, broker registration happens after child spawn, transport setup, service construction, and server construction. That is not the old `supervisor_spawn_unwinds_after_socketpair.rs` window described by the m2 retrospective: socketpair / proxy / `tokio_command` setup succeeded, but failure occurs before child spawn / registration.

This matters because `scope.md:2307-2310` says the resurrected `supervisor_spawn_unwinds_after_socketpair.rs` asserts fd-count baseline plus proxy/private-state cleanup, but does not mention child reap. With the current injection placement, a child likely already exists, so the test either needs child-reap assertions or the injection point needs to move earlier.

Recommended fix: split the hooks by the actual ownership transition:

- **pre-child-spawn / post-socketpair** hook: after socketpair/proxy/sandbox command allocation, before `cmd.spawn()`. This covers fd/proxy/private-state unwind without child reaping.
- **post-register** hook: after `register_plugin` and after whatever reaper/child ownership transition the test intends to exercise. If the test says "via the reaper", specify that the reaper is already installed before injection.

### H2. M1 assigns role/own-topic publish checks to `manifest_standalone`, which lacks the needed identity

`scope.md:2331-2355` says to extend `rafaello_core::validate::manifest_standalone` with role-aware publish rules including `plugin.<own-topic-id>.*` and `provider.<own-id>.*` checks. But standalone manifest validation does not receive a `CanonicalId`, so it cannot derive or compare the manifest's own `plugin.<topic-id>`.

Current code already separates these phases: `manifest_standalone` performs grammar/general checks, while `manifest_with_id(manifest, canonical)` performs own-topic/provider mismatch checks.

Recommended fix: constrain `manifest_standalone` to checks it can actually perform:

- topic grammar and pattern-in-publish-position;
- top-level namespace classification;
- `core.*` / `frontend.*` forbidden for plugin manifests;
- unknown top-level namespace (`evil.foo`) rejection.

Keep own plugin-topic and provider-id matching in `manifest_with_id` (and lock-level equivalents where applicable).

### H3. M1 error variants and baseline test names conflict with the current m1/m2 surface

`scope.md:2358-2377` introduces or references:

- `ValidationError::PublishNamespaceForbidden { topic, namespace }`;
- `ValidationError::PublishNamespaceUnknown { topic, namespace }`;
- `ValidationError::PublishProviderTopicMismatch`;
- an existing positive test named `manifest_publishes_provider_topic.rs`.

Current code/tests use different names:

- `PublishOnReservedNamespace { topic }` for `core.*`;
- `PublishOnFrontendNamespace { topic }` for `frontend.*`;
- `ProviderNamespaceMismatch` for provider-id mismatch;
- existing provider positive coverage is `manifest_parse_provider_example.rs`, while the named `manifest_publishes_provider_topic.rs` file is not present.

This conflicts with `scope.md:2383-2387`, which says existing m1 tests must continue to pass and that the tightening is additive.

Recommended fix: either preserve existing variants and only add the new unknown-namespace variant, or explicitly state that m3 intentionally renames/migrates the validation error surface and updates all affected tests. Also replace the stale baseline test filename with the real current test anchor.

## Medium

### M1. Headless TUI stderr format contradicts parse/render assertions

`scope.md:1797-1802` says headless TUI stderr exposes one JSON-encoded line per received entry so tests can parse and assert without a fake terminal. But `scope.md:1806-1812` specifies human sentinel lines only:

```text
bus.event topic=<topic> seq=<n>
test-done
```

Separately, `scope.md:1277-1279` says the demo bar asserts that all nine entries are rendered, while `scope.md:2589-2597` for `rfl_chat_demo_bar.rs` only asserts SQLite row count/kind/seq/payload.

Recommended fix: choose one contract. If tests need render assertions, make stderr emit JSON including at least topic, seq, entry kind, and rendered tree summary/value. If not, weaken the earlier "renders all of them" claim to match the SQLite-only demo-bar assertion.

### M2. `tui_sends_frontend_ready_after_handler_registration.rs` relies on a timing heuristic

`scope.md:2574-2583` says the test publishes a session event 1 ms after parent-side `frontend.ready` is observed, then infers that seeing the event proves the handler was registered before readiness.

That inference is weak: if the TUI incorrectly sends `frontend.ready` before installing the handler but installs the handler within the 1 ms delay, the test still passes. The 1 ms delay is also scheduler-sensitive under CI/macOS load.

Recommended fix: replace the timing heuristic with a deterministic barrier or remove the delay entirely and publish immediately after observing ready. If the goal is a strict ordering proof, add an explicit test-only "handler installed" sentinel inside the TUI startup path and assert ready is sent only after that sentinel condition is true.

## Low

### L1. Manual validation requires Ctrl+C, but the TUI input spec only names `q`

`scope.md:1832-1833` specifies keyboard handling as `q` to quit and arrow keys to scroll. `scope.md:2823` then requires manual validation of "Ctrl+C / `q` quit cleanly".

In crossterm raw mode, Ctrl+C is just another key event unless explicitly handled; it is not necessarily delivered as normal SIGINT behavior.

Recommended fix: either add Ctrl+C handling to the TUI loop contract or remove Ctrl+C from the manual-validation checklist.

### L2. Round-16 shutdown seam summary disagrees with the detailed signature

The round-16 highlights at `scope.md:9-12` say the seam signature uses `Fn(Pid, Option<Signal>)` for signals plus a separate probe. The detailed snippet at `scope.md:2145-2151` uses:

```rust
mut signal_fn: impl FnMut(Pid, Signal) -> Result<(), Errno>,
mut probe_fn: impl FnMut(Pid) -> Result<(), Errno>,
```

With a separate probe function, the detailed shape is fine; the top summary is stale and could send implementers toward the older `Option<Signal>` design.

Recommended fix: align the summary with the detailed signature: signal function accepts only real `Signal` values (`SIGTERM`/`SIGKILL`), while the no-op liveness probe is represented solely by `probe_fn`.
