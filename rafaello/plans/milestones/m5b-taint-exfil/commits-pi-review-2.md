# m5b commits.md round-2 pi review

> Verdict: blocking.
> Counts: B/3 M/4 N/2

Reviewed round-2 `commits.md` at `654ef61` (user reported `2bee0fe`; current worktree tip is `654ef61`), round-1 review, ratified `scope.md`, Stream A §7.2.1 / §7.2.2 / §7.2.6, and live source under `rafaello/crates/` (`bus.rs`, `reemit/mod.rs`, `gate/mod.rs`, `audit/mod.rs`, `rafaello-tui/src/env.rs`, `rafaello-tui/src/bin/rfl_tui.rs`, `rafaello-openai/src/bin/rfl_openai.rs`).

Round 2 closes most of the mechanical round-1 items, but three per-commit green-bar blockers remain. Two are caused by fixes that name a surface that does not exist or does not have the claimed failure semantics; one is a missing fixture mechanism for the post-spawn PT1 violation test.

## Round-1 verification table

| r1 finding | status | verification |
|---|---|---|
| B1 c08 hook acceptance asserted nonexistent broker event store | closed | c08 now limits row-local acceptance to `Some(err)` suppressing fan-out, `None` permitting fan-out, and last-writer-wins (commits.md:809-839). Handler index assertions are left to c10/c11/c13. |
| B2 c15 tried to publish `core.session.tool_request` with `taint = None` through the broker | partially closed | c15 correctly acknowledges live `publish_core_with_taint` rejects missing taint (commits.md:1584-1589; live `bus.rs`:932-940). However the replacement names `build_confirm_request_payload`, which does not exist in live `gate/mod.rs` (B1 below). |
| B3 c18 parsed plural answers but did not edit runtime dequeue | partially closed | c18 now edits `bin/rfl_tui.rs` and adds a queue helper (commits.md:1838-1853, 1900-1907). The exhaustion acceptance still assumes a detached Tokio task panic exits the process, which live runtime shape does not provide (B2 below). |
| B4 c22/c26 shared-lock mutation broke exact assertions | closed | c22 now ships the final five-plugin lock and c26 consumes it without mutation (commits.md:2153-2230, 2624-2651). c22's catalog assertion is updated to exactly three tool schemas (commits.md:2251-2262). |
| B5 fetch log landed in c23 but c28 depended only on c22 | closed | fetch log emission moves to c21 and env.pass to c22; c23/c28 consume the surface (commits.md:2065-2149, 2173-2184, 2435-2445). |
| M1 missing c11/c12 deps | partially closed | c11 now depends on c08 and c12 now depends on c10 (commits.md:1101-1104, 1182-1185). c13 still omits c11/c12 while its acceptance consumes both (M1 below). |
| M2 c03 withdrawn-variant negative test unbuildable | mostly closed | the negative test is removed (commits.md:338-346). The replacement exhaustiveness wording still leaves the agent choosing between impossible enum iteration and a hand-maintained table (M3 below). |
| M3 integration-test paths used non-live `rafaello/tests` root | closed | paths are normalized to `crates/rafaello/tests/...` / `crates/rafaello/tests/fixtures/...` in the EXFIL/C38 rows (e.g. commits.md:2330, 2468-2474, 2624, 2691-2723). |
| M4 sizing summary under-justified >100 LoC rows | partially closed | c18/c20/c21/c22 now carry row-local justifications, but the sizing summary has a visible 27-vs-28 / 13-vs-14 contradiction (N1). |
| N1 c20 `src/main.rs` vs `src/bin` drift | closed | c20 now has an explicit packaging note (commits.md:2016-2029). |
| N2 c16 left overlay path to the agent | closed | c16 now names live `crates/rafaello-tui/src/confirm.rs` (commits.md:1680-1687, 1729-1731). |

## Blocking findings

### B1 — c15 names `build_confirm_request_payload`, but live `gate/mod.rs` has no such helper

Anchor: c15 (`gate.details.taint` regression) and c17 (`confirm_request_taint_attached`).

c15's replacement for the unreachable broker path says to add only a `#[cfg(test)] mod tests` block and:

> Call `build_confirm_request_payload` directly ... The function is a `pub(super)` or `pub(crate)` helper per m5a's gate layout — accessible ... without modification.

(commits.md:1596-1606; acceptance repeats it at 1643-1648). c17 also refers to the same neighbourhood (commits.md:1745-1748).

Live `crates/rafaello-core/src/gate/mod.rs` does not define `build_confirm_request_payload`; the confirm payload is built inline inside `hold_for_confirmation` (live `gate/mod.rs`:386-401). Therefore c15 cannot be implemented as written with only a test module. The per-commit agent must either invent a helper/extraction refactor not listed in files touched, or call private `hold_for_confirmation` with a full broker/audit/state setup.

Smallest fix: choose one concrete reachable seam. Either (a) make c15 explicitly extract a `build_confirm_request_payload(...) -> Value` helper in `gate/mod.rs`, list the production refactor, and let c17 consume that helper; or (b) keep no refactor and make the in-module unit test call `hold_for_confirmation` with a `BusEvent { taint: None, ... }` plus internal subscriber. Do not cite a helper that is not live.

### B2 — c18's exhaustion test assumes a detached Tokio task panic terminates `rfl-tui`

Anchor: c18 (`RFL_TUI_TEST_CONFIRM_ANSWERS`).

c18 says the plural queue is consumed from `rfl_tui.rs`'s modal-handling loop (commits.md:1838-1850) and acceptance requires:

> drive three modals; assert the `rfl-tui` process exits with the panic-shaped exit code and stderr contains the exhaustion message

(commits.md:1895-1899).

Live `rfl_tui.rs` starts auto-confirm by calling `spawn_auto_confirm_answer(...)` and then awaits `pending()` forever in test mode (live `rfl_tui.rs`:86-95). `spawn_auto_confirm_answer` itself uses `tokio::spawn(async move { ... })` and the `JoinHandle` is dropped (live `rfl_tui.rs`:125-154). A panic inside that detached task does not make the parent `rfl-tui` process exit with a panic-shaped code; the main future remains pending.

Scope §TUI-MA1 requires the exhaustion to be a loud test failure (scope.md:1830-1838). Round 2 still lacks the runtime restructuring needed to make that true.

Smallest fix: c18 must specify how the auto-confirm task failure is surfaced to the process. For example, run plural auto-confirm in a task whose `JoinHandle` is selected/awaited by the main test-mode future, or send a fatal oneshot to the main future and `panic!` there. Then update files touched/tests accordingly. A helper whose `next_answer()` panics is not enough while it is called from a detached task.

### B3 — c22 re-enables the post-spawn PT1 violation test without a registered violating publisher fixture

Anchor: c22 (`m5b fixture lock`) and c14 deferred PT1 test.

c22's final lock contains five plugins: openai, rafaello-fetch, mailcat, readfile, and mockprovider (commits.md:2161-2207). Its acceptance re-enables:

> `rfl_chat_pt1_violation_after_plugin_spawn_writes_audit_row.rs` ... publish a violating `plugin.<id>.tool_result` ... via a test-only bus-fixture publisher

(commits.md:2284-2294).

But no row adds such a publisher to the lock or otherwise registers it with the live broker. Live `rfl-bus-fixture` can publish arbitrary `bus.publish` params only when it is spawned as a registered bus participant in one of its modes (live `rfl_bus_fixture.rs`:135-151, 292-306). The c22 lock does not include `rfl-bus-fixture`, and the existing fetch/mailcat/readfile/mockprovider plugins do not publish a deliberately non-superset `plugin.<id>.tool_result` on demand. An external test process cannot publish on `plugin.<id>.*` unless it is the registered plugin for that topic id.

Smallest fix: add a concrete violation fixture. Options: a dedicated per-test lock replacing `rafaello-fetch` with `rfl-bus-fixture publish_full_params` under the same tool route, a test-only mode in `rafaello-fetch` that publishes a bad taint for one request, or a core-level in-process `rfl chat` harness that exposes the `Broker` handle. The row must name the mechanism and the fixture/env vars; "via a test-only bus-fixture publisher" is not enough.

## Major findings

### M1 — c13's dependency list still omits c11/c12 surfaces used by its own acceptance

Anchor: c13.

c13 declares only:

> Depends on. c09, c10

(commits.md:1299). But c13 acceptance says its first test cites a request id earlier recorded by `handle_tool_request (c11)` with a taint vector that includes `{tool, <fetch>}` (commits.md:1302-1311), and the end-to-end referenced-ancestry test extends c12's manual-seed test with live wiring (commits.md:1293-1296, 1337-1342).

Sequential order happens to put c11/c12 before c13, but the declared dependency graph remains incomplete after the round-2 sweep.

Smallest fix: add c11 and c12 to c13's `Depends on`, or rewrite the c13 acceptance to seed `ReferencedTaintIndex` manually and not claim it consumes c11/c12 live surfaces.

### M2 — c23 still says the stub response encodes `in_reply_to`, but live `rfl-openai` derives it internally

Anchor: c23 EXFIL1 fixture description.

c23 says the stub scripted response includes turn-2 `tool_calls` with:

> `in_reply_to = [<turn-1 fetch-tool_result-request_id>]`

(commits.md:2345-2354, 2468-2474). Live OpenAI wire `ToolCall` has only `id`, `type`, and `function` (live `wire.rs`:53-59); unknown fields in the stub JSON would be ignored by serde. Live `rfl-openai` publishes provider `tool_request.in_reply_to` from `state.observed_tool_results`, not from the model/stub response (live `rfl_openai.rs`:329-341).

The intended test shape is valid — after turn 1's tool_result is observed, turn 2 should cite it — but the row attributes that surface to the wrong fixture. A per-commit agent may waste time adding ignored JSON fields and then wonder why they do not control the bus envelope.

Smallest fix: say the stub only scripts the two tool calls; `rfl-openai` itself derives turn-2 `in_reply_to` from observed tool results. The acceptance should assert the published provider event's `in_reply_to`, not claim the JSON fixture carries it.

### M3 — c03's replacement exhaustiveness test is still underspecified

Anchor: c03 (`AuditKind` table extension).

Round 2 correctly deletes the unbuildable negative test, but replaces it with:

> compute the set difference between the post-m5b `as_str()` outputs (collected by iterating all variants — or by maintaining a `pub const M5B_NEW_KIND_STRS: [&str; 3]` table in the test) and a snapshot of the pre-m5b set

(commits.md:347-359).

Live `AuditKind` has no iterator/strum support (scope §AL4 explicitly says only `as_str()` exists; scope.md:1959-1963). The "or" leaves a design choice to the per-commit agent, and one branch is not implementable without adding new enum-iteration machinery outside §AL4.

Smallest fix: pin the simple implementable test: instantiate exactly the three new variants and assert their `as_str()` values; optionally assert a static array of those three strings has length 3 and contains no withdrawn name. Do not ask the agent to iterate all enum variants unless the row also adds an iterator.

### M4 — c08's production-absence compile fence still leaves the harness choice to the agent

Anchor: c08 (`Broker::install_publish_test_hook`).

c08's production-absence acceptance still says the implementation may use `build.rs`, a `cargo check` invocation, a `trybuild` compile-fail directory, or fall back to a cfg-gated compile-pass file plus a documentation comment if `trybuild` is not present (commits.md:840-853, 857-861).

That is exactly the kind of design branch m1's inline-row rule tries to avoid: different agents will choose different harnesses, and adding `trybuild = "1"` as an unplanned dev dependency changes the row's dependency surface. Scope §TM4 only needs a crisp compile fence that the method is absent outside `test-fixture`/test builds (scope.md:905-911).

Smallest fix: choose one concrete harness. Either add `trybuild` explicitly in c08 and state the exact compile-fail file, or avoid new deps and use an existing cargo/check pattern already present in the repo. Remove the fallback branch.

## Nit findings

### N1 — sizing summary still contains contradictory arithmetic

The sizing summary says `medium ... 13 commits` but lists fourteen ids (c05, c06, c09, c12, c13, c16, c18, c20, c21, c22, c24, c26, c27, c28), then computes `6 + 5 + 13 + 2 + 1 = 27`, then immediately recounts with medium = 14 to get 28 (commits.md:2885-2927). The final total is clear, but leave only the correct count in round 3.

### N2 — cross-checks have stale round-1 wording after the c15/c21/c22 changes

The cross-check still says c15 is a tests-only exception (commits.md:2844-2849), but c15 now edits `gate/mod.rs` with a `#[cfg(test)]` module (commits.md:1670-1675). The topic/env spelling checklist also omits the newly introduced `RFL_FETCH_TEST_LOG_PATH` while claiming all env-var spellings were checked (commits.md:2817-2832). Update the cross-check so it remains useful as a drift detector.

## Convergence call

Round 2 should not converge. The prior round's broad problems are mostly fixed, but c15, c18, and c22 still have per-commit acceptance that cannot be implemented against the live surfaces as written. I expect a smaller round 3: pick the gate payload test seam, make plural-answer exhaustion actually fail the process, and name the PT1 violating publisher fixture.