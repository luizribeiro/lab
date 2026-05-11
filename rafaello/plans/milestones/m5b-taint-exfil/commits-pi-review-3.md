# m5b commits.md round-3 pi review

> Verdict: blocking.
> Counts: B/4 M/2 N/2

Reviewed round-3 `commits.md` at worktree tip `41790c8` (user reported `01ec7cc`), round-2 review, ratified `scope.md`, Stream A §7.2.1 / §7.2.2 / §7.2.6, and live source under `rafaello/crates/` (`bus.rs`, `reemit/mod.rs`, `gate/mod.rs`, `audit/mod.rs`, `rafaello-tui/src/bin/rfl_tui.rs`, `rafaello-openai/src/bin/rfl_openai.rs`, `rafaello-mailcat/src/bin/rfl_mailcat.rs`).

Round 3 closes the named round-2 items on paper, but the new fixes introduced fresh handoff risks. The biggest one is `rafaello-fetch`: c20/c21 still describe a plugin architecture (`Handler`, `run_plugin`, `peer.publish`) that does not match the live bundled plugins, and c20 omits the `fittings-client` dependency needed by that shape. There are also two integration-test mechanics issues: the c18 fatal-path `select!` can still miss the fatal message, and the PT1 violation test does not script/avoid the required sink confirmation.

## Round-2 verification table

| r2 finding | status | verification |
|---|---|---|
| B1 c15 named nonexistent `build_confirm_request_payload` | closed | c15 now explicitly extracts the helper from `hold_for_confirmation` and lists the refactor in `gate/mod.rs` (commits.md:1677-1711, 1744-1784). |
| B2 c18 detached-task panic would not exit `rfl-tui` | partially closed | c18 adds a fatal oneshot and main-future `select!` (commits.md:1960-1999), but the written pseudocode can race into the `JoinHandle Ok(())` arm and then pend forever after sending fatal (B2 below). |
| B3 c22 lacked a registered violating publisher fixture | partially closed | c21/c22 add `RFL_FETCH_TEST_TAINT_OVERRIDE` as the violating-publisher mechanism (commits.md:2256-2277, 2395-2406, 2507-2532). The end-to-end c22 test still omits how the sink confirmation is allowed (B3), and the fetch plugin architecture itself is not live-compatible (B1). |
| M1 c13 dependency omitted c11/c12 | closed | c13 now depends on c09, c10, c11, c12 (commits.md:1390-1394). |
| M2 c23 attributed `in_reply_to` to the stub JSON | partially closed | c23 now correctly says live `rfl-openai` derives `in_reply_to` from `state.observed_tool_results` (commits.md:2578-2597, 2720-2730). It adds a raw provider-event assertion without an observation seam (B4). |
| M3 c03 exhaustiveness test underspecified | closed | c03 now instantiates exactly the three new variants and uses a local `[&str; 3]` table; no enum iteration / `strum` (commits.md:420-444). |
| M4 c08 compile-fence left harness choice open | closed | c08 now commits to `trybuild`, an explicit compile-fail path, and a dev-dependency addition (commits.md:922-946). |
| N1 sizing arithmetic contradictory | closed | sizing now uses medium = 14 and total = 28 once (commits.md:3137-3183). |
| N2 stale cross-check wording | closed | cross-check adds `RFL_FETCH_TEST_LOG_PATH` / `RFL_FETCH_TEST_TAINT_OVERRIDE` and updates c15 from tests-only to helper extraction (commits.md:3084-3090, 3106-3113). |

## Blocking findings

### B1 — c20/c21 describe a `rafaello-fetch` plugin shape that does not match live bundled plugins

Anchor: c20/c21 (`rafaello-fetch` scaffold + handler).

c20's dependencies for the new crate omit `fittings-client` (commits.md:2132-2138). c21 then says the library implements a fittings `Handler` trait and the bin uses a `run_plugin(WebFetchHandler::new())` shape (commits.md:2234-2238, 2285-2288), and its taint override uses a `peer.publish(...)` call (commits.md:2262-2265).

Live bundled plugins do not use that API. `rafaello-mailcat` imports `fittings_client::{Client, InboundNotification}` (live `rfl_mailcat.rs`:14), subscribes to `bus.event`, and publishes results via `peer.notify("bus.publish", json!(...))` (live `rfl_mailcat.rs`:93-103). There is no in-repo `run_plugin` helper, and no `PeerHandle::publish` call pattern in the live plugins. With c20 as written, c21 either cannot compile or must invent a different plugin architecture and add dependencies not listed in c20.

Smallest fix: make c20/c21 mirror an actual live plugin. Add `fittings-client = { workspace = true }` to c20 dependencies, make c21's bin adopt `RFL_BUS_FD`, connect a `Client`, subscribe to `bus.event`, and publish `bus.publish` notifications exactly like mailcat/readfile/openai. Keep `WebFetchHandler` as a pure library helper if desired, but do not claim a nonexistent `Handler` / `run_plugin` surface.

### B2 — c18's fatal oneshot can race with the `JoinHandle Ok(())` arm and hang instead of failing

Anchor: c18 plural-answer fatal surfacing.

Round 3 fixes the detached-task problem by adding a fatal oneshot, but the pseudocode says `next_answer()` sends fatal and returns `None` (commits.md:1940-1953), then the main future selects on both the join handle and `fatal_rx` (commits.md:1978-1994). The `JoinHandle` arm treats `Ok(())` as normal and awaits `pending()` forever (commits.md:1979-1984).

On exhaustion, the queue task can send the fatal message and then return `Ok(())`; both the `fatal_rx` and `join_handle` may be ready. If `tokio::select!` chooses the join arm, the process hangs instead of panicking, so `tui_runtime_confirm_answers_exhaustion_terminates_process.rs` can still flake or fail.

Smallest fix: make exhaustion unambiguously fatal. For example, after sending fatal, the queue task should `panic!` so the join arm is `Err(join_err)`, or the `Ok(())` join arm should first drain/check `fatal_rx` before pending. Pin the exact behaviour; do not leave a ready-branch race.

### B3 — c22's PT1 violation test does not script the required `web-fetch` confirmation

Anchor: c22 re-enabled `rfl_chat_pt1_violation_after_plugin_spawn_writes_audit_row.rs`.

The c22 end-to-end PT1 test drives `web-fetch` through the m5b lock and says:

> the gate allows the fetch dispatch (no other modal scripting)

(commits.md:2517-2520). But `web-fetch` is declared with `sinks = ["network"]` in the same lock (commits.md:2389-2400), so the gate holds it for confirmation. Without a grant or a scripted TUI answer (`RFL_TUI_TEST_CONFIRM_ANSWER=allow` or plural equivalent), the fetch plugin never runs and therefore never publishes the taint-override `tool_result`.

Smallest fix: specify the allow mechanism in the c22 test setup. Either set `RFL_TUI_TEST_CONFIRM_ANSWER=allow` for this single-modal test, or pre-grant `web-fetch` via the existing grant-before-message hook and assert no modal. The row must say which path it uses.

### B4 — c23 adds a raw `provider.openai.tool_request` assertion without an observation seam

Anchor: c23 EXFIL1.

c23 now correctly says the stub JSON does not carry `in_reply_to`; live `rfl-openai` derives it. But the new assertion says to:

> inspect the published `provider.openai.tool_request` event for turn 2

(commits.md:2640-2648). The headline test is an `rfl chat` process integration test (commits.md:2561-2566). No row adds a subscriber/observer fixture to the m5b lock for `provider.openai.tool_request`, and the TUI/frontend only subscribes to `core.session.**` / lifecycle topics. Live `rfl-bus-fixture` has observer modes (live `rfl_bus_fixture.rs`:135-151), but it is not in the c22 lock.

The underlying behaviour is already covered by c12/c13 re-emit tests and by the resulting canonical taint in the c23 modal. If c23 really must assert the raw provider event, it needs a named observation seam (observer plugin in a per-test lock, process-local broker harness, or rfl-openai log hook). Otherwise remove the raw provider-event assertion from the headline integration test and assert the canonical consequence.

## Major findings

### M1 — c15's helper signature still delegates too much to the per-commit agent

Anchor: c15 helper extraction.

c15's new helper is specified as:

```rust
pub(crate) fn build_confirm_request_payload(
    event: &BusEvent,
    // ...other inputs the inline block
    // currently reads from local state...
) -> serde_json::Value;
```

(commits.md:1690-1696). The direction is good, but per-commit prompts are inlined; `// ...other inputs...` leaves the API shape to the implementation agent and to c17, which now consumes the helper by name (commits.md:1856-1868).

Smallest fix: spell out the exact parameters (`confirm_id`, `held_tool_request_id`, `dispatch_target`, `tool`, `args`, `sinks`, `always_confirm`, `ttl_seconds`, etc.) or explicitly say the helper stays private and c17 does not depend on its signature. Do not leave an ellipsis in a commit-plan API cutover.

### M2 — c21's taint-override test mode needs an explicit fixture-only contract

Anchor: c21 taint override.

The new `RFL_FETCH_TEST_TAINT_OVERRIDE` mode is a pragmatic way to test PT1, but it is outside scope §TF2's file-backed fetch semantics. It should be marked clearly as `#[cfg(any(test, feature = "test-fixture"))]` / fixture-only behaviour, or as a documented test-fixture env var accepted in production binaries but only used by tests. Right now it is described as normal plugin behaviour (commits.md:2256-2277) and added to the shared m5b lock (commits.md:2395-2406).

Smallest fix: pin whether this code exists in production builds. If fixture-only, add the cfg and compile coverage. If always compiled, call it an intentional m5b test-fixture escape hatch and add it to the manual/cross-check wording as such.

## Nit findings

### N1 — c22's `What` heading omits `RFL_FETCH_TEST_TAINT_OVERRIDE`

The c22 subject/heading still says env.pass for only `RFL_FETCH_TEST_BODY_PATH` + `RFL_FETCH_TEST_LOG_PATH` (commits.md:2372-2373), but the row now also pins `RFL_FETCH_TEST_TAINT_OVERRIDE` (commits.md:2395-2406). Update the heading so row scanning catches the test-fixture env var.

### N2 — c08's `trybuild` wording over-specifies `cargo check --no-default-features`

c08 says the `trybuild::TestCases` invocation runs `cargo check --no-default-features` (commits.md:927-933). `trybuild` normally compiles its fixture crate as configured by the test harness; if the plan depends on a custom cargo invocation, say how it is wired. If not, simply say the fixture imports `rafaello-core` without `test-fixture` and the `.stderr` proves the method is absent.

## Convergence call

Round 3 should not converge. The remaining issues are narrower than round 2, but c20/c21's fetch-plugin architecture must be corrected before the fixture rows are implementable, and c18/c22/c23 still need concrete test mechanics. I expect round 4 can converge if those are fixed without expanding the commit count.