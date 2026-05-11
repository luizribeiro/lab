# m5b commits.md round-4 pi review

> Verdict: blocking.
> Counts: B/3 M/2 N/1

Reviewed round-4 `commits.md` at worktree tip `50257b6` (`docs(rafaello-m5b): commits.md round 4 — fold pi-review-3 (4B/2M/2N), CONVERGED`). Note: the request cited `fc877a4`, but the checked-out repository tip is `50257b6`. Compared against `commits-pi-review-3.md`, ratified m5b scope, and live source in `rafaello/crates/`.

Round 4 materially improves the plan, but it is not converged. The fetch plugin row still names non-live client APIs, the fixture-only feature path is not actually wired for spawned plugin binaries, and the c22 grant-injection example does not match the live structural grant matcher.

## Round-3 verification table

| r3 finding | status | verification |
|---|---|---|
| B1 c20/c21 fetch plugin used nonexistent `Handler` / `run_plugin` / `peer.publish` shape | partially closed | c20 adds `fittings-client` and c21 rewrites toward a bus-client bin (commits.md:2275-2289, 2451-2488). However c21 still names `Client::new(fd)`, `peer.notify("bus.subscribe", ...)`, and `client.recv()` (commits.md:2456-2470), none of which match live mailcat/client APIs; see B1. |
| B2 c18 fatal oneshot could race with `JoinHandle Ok(())` and hang | closed | c18 now says the queue sends fatal then panics, making the join arm `Err(JoinError)` on exhaustion (commits.md:2057-2072, 2088-2124), with acceptance checking both panic and fatal message (commits.md:2160-2171). |
| B3 c22 PT1 violation test did not script/avoid required sink confirmation | partially closed | c22 now specifies `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE` and zero modal assertions (commits.md:2739-2784). But the provided grant template uses `{"url": ""}`, which does not match the actual fetch URL under live structural matching; see B3. |
| B4 c23 raw `provider.openai.tool_request` assertion lacked an observation seam | closed | c23 drops the raw provider-event assertion and limits the headline test to canonical consequences (commits.md:2902-2917). |
| M1 c15 helper signature delegated too much to agent | partially closed | c15 now spells an explicit signature (commits.md:1785-1801), but immediately permits the implementation agent to adjust it if live source reads a different subset (commits.md:1803-1811); see M1. |
| M2 `RFL_FETCH_TEST_TAINT_OVERRIDE` needed fixture-only contract | partially closed | c21 adds `#[cfg(any(test, feature = "test-fixture"))]` gates and a compile-fence row (commits.md:2413-2443, 2560-2569). Runtime feature enablement for spawned plugin binaries is still not wired; see B2. |
| N1 c22 heading omitted `RFL_FETCH_TEST_TAINT_OVERRIDE` | closed | c22 heading now includes all three env vars (commits.md:2591). |
| N2 c08 over-specified `cargo check --no-default-features` | closed | c08 now describes a normal `trybuild` fixture without a custom cargo invocation (commits.md:1020-1036). |

## Blocking findings

### B1 — c21 still does not mirror the live bus-client API

Anchor: c21 `rafaello-fetch` bin.

Round 4 removes the nonexistent `Handler` / `run_plugin` design, but the replacement still names APIs and protocol steps that are not live:

- c21 says to construct `fittings_client::Client::new(fd)` (commits.md:2456-2460). Live mailcat parses `RFL_BUS_FD`, calls `adopt_bus_fd`, then `Client::connect(OneShotConnector::new(transport)).await` (live `rfl_mailcat.rs`:36-50). `fittings_client::Client` exposes `connect`, not a file-descriptor `new` constructor.
- c21 says to subscribe via `peer.notify("bus.subscribe", ...)` (commits.md:2461-2468). Live mailcat does not send a bus-subscribe notification; it calls `client.subscribe_notifications()` and relies on supervisor/broker fan-out from registered ACLs (live `rfl_mailcat.rs`:51-58). The live supervisor service routes only `bus.publish` and `core.tools_list`; every other method returns `MethodNotFound` (live `supervisor.rs`:1115-1119, 1165-1175).
- c21 says the main loop awaits `client.recv()` (commits.md:2469-2470). The live client surface is the broadcast receiver returned by `subscribe_notifications()`, not `Client::recv()`.

Smallest fix: make c21 say “copy the mailcat bin shape” literally: `parse_bus_fd`, `adopt_bus_fd`, `OneShotConnector`, `Client::connect`, `subscribe_notifications`, loop on `notifications.recv().await`, and no `bus.subscribe` notify. Also add any dependency needed by the copied shape (notably `ulid`, if using mailcat’s fresh request-id pattern).

### B2 — c21/c22 gate log/override code behind a feature but do not wire that feature for spawned plugin binaries

Anchor: c21 fixture-only env vars and c22 runtime tests.

c21 gates both `RFL_FETCH_TEST_LOG_PATH` and `RFL_FETCH_TEST_TAINT_OVERRIDE` behind `#[cfg(any(test, feature = "test-fixture"))]` and says production builds do not compile/read them (commits.md:2413-2443). c22 then relies on those exact branches in spawned plugin processes: the lock passes `RFL_FETCH_TEST_LOG_PATH` / `RFL_FETCH_TEST_TAINT_OVERRIDE`, and the end-to-end tests assert fetch logs and PT1 violating publishes (commits.md:2718-2726, 2731-2776).

The plan says the lock builds the fetch plugin with `test-fixture` via a workspace aggregator feature (commits.md:74-82), but the live workspace has no such aggregator in the virtual `rafaello/Cargo.toml` (live `rafaello/Cargo.toml`:1-3, 40-44), and c22 does not add lock/supervisor/build metadata that can enable Cargo features for a spawned binary. A binary built in normal mode will therefore ignore both env vars, making c22’s log-path and PT1-override assertions fail.

Smallest fix: pin the actual feature-enable path. For example: add a real feature aggregation on the package/test that builds the fixture binary, state the exact `cargo test` feature invocation, or avoid feature-gating the spawned binary branches and instead gate only tests/seams by fixture package selection. The c22 tests must prove the spawned binary has the feature enabled.

### B3 — c22’s grant-injection template does not match the live structural matcher

Anchor: c22 re-enabled PT1 violation test.

c22 now uses `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE`, but the specified template is:

```json
{"tool": "web-fetch", "args_subset": {"url": ""}}
```

(commits.md:2739-2745). Live slash parsing turns `args_subset` into a structural grant template (live `slash.rs`:193-208), and live `UserGrants::matches` performs recursive exact-value subset matching: scalar leaves must be equal (`t == a`) (live `user_grants.rs`:101-132). The planned openai stub calls `web-fetch` with a real URL (`https://content.example.com/page` elsewhere in the plan; commits.md:2828-2830), so `{"url": ""}` will not match and the gate will still emit a confirmation request. That contradicts c22’s “zero confirm_request audit rows” assertion (commits.md:2781-2784).

Smallest fix: use an exact matching URL template, e.g. `{"url":"https://content.example.com/page"}`, or change the grant schema/test hook to support an explicit wildcard and test that wildcard. Do not cite mere JSON-schema validity as sufficient for grant-match short-circuiting.

## Major findings

### M1 — c15 still weakens the supposedly explicit helper signature

c15 spells out `build_confirm_request_payload(...)` (commits.md:1791-1801), then says the implementation agent may adjust the helper signature if the live block reads a different subset (commits.md:1803-1811). That reintroduces the API handoff ambiguity round 3 flagged. Either pin the exact signature, or say c17 consumes only the helper return value and does not depend on a fixed helper signature.

### M2 — c21’s compile-fence cannot prove private fixture functions are absent

c21’s trybuild fixture references `take_taint_override()` or `maybe_write_invocation_log(...)` from outside the crate (commits.md:2560-2569), but those functions are written as private `fn`s (commits.md:2413-2431). An external compile-fail fixture will fail even when the feature is enabled because the functions are private, so the test does not prove cfg absence. Expose a tiny cfg-gated public sentinel for the compile fence, or run the compile-fail test inside the crate/module boundary.

## Nit findings

### N1 — Request hash mismatch should be noted in the next round metadata

The request cited round-4 commit `fc877a4`, but the checked-out repository shows `50257b6` as the round-4 `commits.md` tip. If this branch is expected to have `fc877a4`, rebase/checkout drift should be resolved before final ratification.

## Convergence call

Not converged. Round 4 closes the conceptual round-3 issues, but the implementation handoff still has blocking mechanics in the fetch plugin and PT1 test path. Fixing B1-B3 without expanding scope should be enough for a short round 5.