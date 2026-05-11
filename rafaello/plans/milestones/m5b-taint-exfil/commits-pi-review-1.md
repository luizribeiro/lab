# m5b commits.md round-1 pi review

> Verdict: blocking.
> Counts: B/5 M/4 N/2

Reviewed `commits.md` round 1 at `a1359fb`, ratified `scope.md` round 7, `driver-preflight.md`, m5a `commits.md` + pi review tradition, `plans/README.md` prior-milestone patterns, Stream A §7.2.1 / §7.2.2 / §7.2.6, and live source under `rafaello/crates/` (`bus.rs`, `reemit/mod.rs`, `gate/mod.rs`, `audit/mod.rs`, `rafaello-tui/src/env.rs`, `rafaello-tui/src/bin/rfl_tui.rs`, `rafaello/src/lib.rs`).

The draft is close on overall scope coverage and the 28-default budget, but it is not yet handable to per-commit agents. The main failures are per-commit green-bar mechanics: one test seam has an impossible row-local acceptance, one test-only gate row cannot drive the stated input through the live broker, the plural TUI answer row omits the runtime file that must consume the parsed queue, the main m5b lock is mutated later in a way that breaks c22's own exact assertions, and fetch-log instrumentation is introduced too late for later rows that depend only on c22.

## Round-1 verification table — blockers

| id | row(s) | status | core problem |
|---|---|---|---|
| B1 | c08 | open | hook-row acceptance asserts broker state that live `publish_core_with_taint` does not have |
| B2 | c15 | open | test-only integration row asks to publish `core.session.tool_request` with `taint = None`, which live broker rejects |
| B3 | c18 | open | row scopes runtime dequeue/exhaustion but touches only `env.rs`; live dequeue is in `bin/rfl_tui.rs` |
| B4 | c22/c26 | open | c26 mutates the shared m5b lock to add readfile, breaking c22's exact-two tool-catalog test |
| B5 | c23/c28 | open | fetch-log instrumentation/env pass lands in c23, but c28 depends only on c22 while asserting `fetch.log` |

## Blocking findings

### B1 — c08's hook acceptance is not implementable against live `publish_core_with_taint`

Anchor: c08 (`Broker::install_publish_test_hook`).

c08 says the hook is consulted:

> after writing the event payload but **before** `fan_out` runs ... The hook's `&BusEvent` argument is the post-record event so the test can inspect index state at hook-fire time.

and its row-local test asserts a captured broker `state` snapshot proves the event payload was written / post-record state exists (commits.md:697-705, 725-734).

Live `Broker::publish_core_with_taint` has no broker-side event store. It validates, constructs a local `BusEvent`, immediately calls `self.fan_out(&event, ...)`, and returns (live `bus.rs`:968-977). There is no "event payload was written" state to snapshot in c08. Scope §TM4's useful assertion is about handler-owned indexes after c10/c11 record calls and before fan-out (scope.md:837-845, 893-900); c08 can provide that seam, but c08 itself cannot prove handler-recorded state before those handlers consume it.

Smallest fix: make c08's own acceptance only prove hook ordering relative to fan-out and last-writer-wins (`Some(err)` suppresses subscriber delivery; `None` permits delivery). Leave `TaintMatchMap` / `ReferencedTaintIndex` state assertions to c10/c11/c13, where the handlers and indexes exist.

### B2 — c15 is tests-only but its first integration test cannot drive `taint = None` through the live broker

Anchor: c15 (`gate.details.taint` regression tests).

c15 explicitly declares a tests-only exception and says it touches no production code (commits.md:1479-1504, 1530-1534). Its first acceptance says to:

> drive a `core.session.tool_request` with `taint = None`; observe the gate's `confirm_request` payload; assert `details.taint == json!([])`

(commits.md:1512-1516; scope CD1 at scope.md:1745-1766).

But the live broker rejects `core.session.tool_request` with missing taint before the gate can see it: `publish_core_with_taint` returns `InvalidTaint { reason: Missing }` for `core.session.tool_request` when `taint` is `None` (live `bus.rs`:932-940). The fallback under test is inside the private gate helper (`event.taint.clone().unwrap_or_default()`, live `gate/mod.rs`:386-398), not reachable from an integration test in `crates/rafaello-core/tests/` without adding a seam or moving the test into the module.

Smallest fix: either (a) make c15 add an explicit gate-local test seam / in-module unit test and say it touches `gate/mod.rs` test code, or (b) change the acceptance to a reachable live shape (e.g. provider-only non-empty taint) and drop the impossible `None` integration assertion. Do not leave c15 as a no-code integration-test row with an input the broker refuses.

### B3 — c18 cannot implement the plural-answer runtime queue by editing only `env.rs`

Anchor: c18 (`RFL_TUI_TEST_CONFIRM_ANSWERS`).

Scope §TUI-MA1 requires both parsing and runtime consumption: the parsed vector is mirrored by a queue and dequeued on each confirm modal, with exhaustion panic (scope.md:1804-1838). c18 repeats that requirement (commits.md:1661-1690) but its files-touched line is only:

> `crates/rafaello-tui/src/env.rs` (parser + queue + mutex check, ~80 lines); four new test files.

(commits.md:1723-1725).

Live runtime auto-confirm consumption is not in `env.rs`. `rfl_tui.rs` checks only `cfg.test_confirm_answer` and calls `spawn_auto_confirm_answer` with one `TestConfirmAnswer` (live `crates/rafaello-tui/src/bin/rfl_tui.rs`:86-94); the loop then reuses that one answer for every modal (lines 125-154). Without editing `src/bin/rfl_tui.rs` (or moving the runtime into a shared module consumed from there), c18 cannot satisfy either `round_trip_two_answers` or `exhaustion_panics`.

Smallest fix: c18 must list and edit the runtime consumer (`crates/rafaello-tui/src/bin/rfl_tui.rs`, or a new helper module plus the bin call site) in the same commit as the parser. Keep the parser-only unit tests, but add the queue-consumption tests against the actual modal path.

### B4 — c26's late readfile addition breaks c22's exact-two tool-catalog acceptance

Anchors: c22 (`m5b fixture lock`) and c26 (`five-tree spawn`).

c22's lock acceptance asserts the combined lock's `ToolSchemaCatalog::list()` contains exactly two tool schemas: `web-fetch` and `send-mail`; openai + mockprovider contribute none (commits.md:1967-1977). Later, c26 decides to mutate that same shared `rafaello/fixtures/m5b-locks/rafaello.lock` by appending `rfl-readfile` as the fifth plugin (commits.md:2332-2358) to satisfy scope §C38a's five-tree test (scope.md:2290-2303).

After c26 lands, the c22 regression test still runs in the full workspace. If the appended readfile entry is a normal `rfl-readfile` fixture, it contributes a `read_file` tool schema, so c22's "exactly two" assertion becomes false. c26 says only "lock edit compiles" (commits.md:2368-2375); it does not amend c22's exact-two test.

Smallest fix: do not mutate the shared c22 lock after exact assertions land. Either make c22 ship the final five-plugin lock and assert exactly three tool schemas (or explicitly filter sink tools if that is the intended invariant), or make c26 use a per-test five-plugin lock variant so c22's four-plugin catalog test remains stable.

### B5 — fetch-log instrumentation is introduced in c23, but c28 depends only on c22 while asserting `fetch.log`

Anchors: c23 (`EXFIL1`) and c28 (`C38c`).

Scope EXFIL1 expects a fetch invocation log (scope.md:2202-2205). The commit plan chooses to implement that log in c23, not in the fetch fixture rows:

> the file path passed via a `RFL_FETCH_TEST_LOG_PATH` env var — added to the m5b lock's env.pass list inside this commit ... c23 owns the log-emission addition

(commits.md:2134-2154), and c23's files touched include `crates/rafaello-fetch/src/lib.rs` plus a possible m5b lock env-pass extension (commits.md:2197-2205). By contrast, c21's TF2 handler only reads `RFL_FETCH_TEST_BODY_PATH` (commits.md:1856-1863), and c22's TF3 lock pins only `RFL_FETCH_TEST_BODY_PATH` (scope.md:2033-2035; commits.md:1957-1964).

c28 then asserts `fetch.log` has one entry but declares only `Depends on. c22` (commits.md:2412-2440). If a driver ever cherry-picks or repairs c28 from its declared deps, the fetch log surface does not exist. Even in strict sequential order, c28's row metadata lies about the surface it consumes.

Smallest fix: put `RFL_FETCH_TEST_LOG_PATH` support, env.pass, and a small unit/fixture test in c21/c22 (where the fetch plugin and lock surfaces land), or add `c23` as a dependency to c28 and state that c28 consumes c23's test-fixture instrumentation. Prefer the former: c23 should stay a headline integration test, not the commit that mutates fixture-plugin behaviour for later rows.

## Round-1 verification table — majors

| id | row(s) | status | core problem |
|---|---|---|---|
| M1 | c11/c12 | open | declared dependencies omit surfaces used by acceptance tests |
| M2 | c03 | open | extra negative enum-variant test is not a normal implementable Rust test and is outside AL4 acceptance |
| M3 | c02/c19/c23-c28 | open | integration-test paths use non-live `rafaello/tests/...` root |
| M4 | sizing | open | several rows exceed the stated <100 LoC / <5 file guideline while being classified as small/medium without row-local justification |

## Major findings

### M1 — c11 and c12 omit declared dependencies they use in acceptance

Anchors: c11 and c12 dependency lines.

c11's acceptance uses c08's `install_publish_test_hook` twice (commits.md:1004-1015), but c11 declares only `Depends on. c09` (line 1001). c12's value-driven acceptance records a `tool_result` via c10's `handle_tool_result` path (commits.md:1083-1088), but c12 declares `Depends on. c03, c06, c09, c11` (line 1080), omitting c10.

Phase order currently places c08 and c10 earlier, so this is not an immediate sequential-run compile break, but the dependency graph is wrong for driver recovery and per-row handoff. m5a review treated this class as a major when a row extended another row's tests without declaring the dependency.

Smallest fix: add c08 to c11's dependencies and c10 to c12's dependencies. If c12 intentionally avoids live c10 wiring by manually seeding the map, update the acceptance text to remove "via c10's `handle_tool_result` path".

### M2 — c03's withdrawn-variant negative test is outside scope and not a normal Rust test

Anchor: c03 (`AuditKind` table extension).

Scope §AL4 asks for one table-driven `as_str()` test over the three new variants (scope.md:1949-1970). c03 adds a second acceptance:

> assert via a `match` arm exhaustiveness pattern that `tool_request_rejected_taint_superset` is **not** a variant

(commits.md:271-275).

A normal Rust integration test cannot reference a non-existent enum variant and still compile. This needs a `trybuild` compile-fail harness, a source-text grep (not ideal), or, better, no test at all: the positive `as_str()` table and the absence of any producer already pin the withdrawn name. As written, the per-commit agent will either create an unbuildable test or invent a compile-fail framework not scoped by c03.

Smallest fix: delete the negative test acceptance or rewrite it as an ordinary positive assertion that the `as_str()` output set is exactly the three scoped strings.

### M3 — many integration-test paths do not match the live workspace layout

Anchors: c23-c28 and related rows.

Rows c23-c28 name tests under `rafaello/tests/...` and fixtures under `rafaello/tests/fixtures/...` (examples: commits.md:2025-2027, 2177-2201, 2224-2225, 2284-2285, 2334-2335, 2414-2447). Live package integration tests are under `rafaello/crates/rafaello/tests/`; there is no root `rafaello/tests` directory in this worktree. The same mismatch appears in smaller rows such as c19 (`Tests in rafaello/tests/`, commits.md:1739-1747).

m5a plans sometimes used shorthand, but per-commit prompts are inlined and this draft already gives precise `crates/...` paths elsewhere. Leaving the wrong root invites agents to create non-package files that Cargo will not run.

Smallest fix: normalize these to `crates/rafaello/tests/...` and `crates/rafaello/tests/fixtures/...` (or explicitly state that `rafaello/tests` is shorthand for the `crates/rafaello` package test directory, but precise paths are safer).

### M4 — the sizing summary dilutes the stated commit-size rule

Anchor: header and sizing summary.

The header restates the operational guideline: prefer `<3-5 files / <100 lines per row`, with only c04, c13, c14, and c23 called out as body-justified larger (commits.md:13-32). The sizing summary then redefines `medium` as 100-300 LoC and includes 13 rows in that bucket (commits.md:2596-2608). c20 is a concrete mismatch: it touches the workspace manifest, seven new fixture/crate files, a bin shim, and a test file, and is still labelled `small (~150 LoC...)` (commits.md:1766-1845).

This does not mean every medium row must split, but rows that exceed both the file and line guideline need row-local justification or a split rationale. Otherwise the sizing section stops being useful as a drift signal.

Smallest fix: either split the worst offenders (especially scaffold + many tests rows) or add explicit row-local body justifications for each >100-LoC / >5-file row that remains bundled. Keep the top-level "only four rows" list in sync.

## Nit findings

### N1 — c20's TF1 shape drifts from scope's `src/main.rs` wording

Scope §TF1 says the new crate has `src/main.rs` with the fittings `run_plugin(handler)` shape (scope.md:1974-1999). c20 instead scaffolds `src/bin/rafaello_fetch.rs` first and fills the fittings `run_plugin` shape in c21 (commits.md:1776-1791, 1870-1873). This is probably fine Rust packaging, but the row should acknowledge the intentional `src/bin` split so scope traceability does not look accidental.

### N2 — c16 leaves the TUI overlay file path to the agent

c16 says to edit `confirm_overlay.rs` "or m5a's overlay file wherever ... lives — driver agent reads to confirm path" (commits.md:1540-1545). Live source has the overlay renderer in `crates/rafaello-tui/src/confirm.rs`. Per m1's inline-prompt rule, commit rows should not leave file discovery/design choices to the per-commit agent when the live path is known.

## Convergence call

Round 1 should not converge. The plan's broad scope mapping is good, but c08, c15, c18, c22/c26, and c23/c28 need mechanical fixes before implementation can proceed with per-commit green bars. After those are fixed, re-attack the dependency graph and sizing table; I expect at least one more blocking round focused on the fixture-lock and EXFIL test surface.