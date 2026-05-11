# m5b commits.md round-6 pi review

> Verdict: CONVERGED.
> Counts: B/0 M/0 N/0

Reviewed round-6 `commits.md` at worktree tip `2f45536` against `commits-pi-review-5.md`, live source under `rafaello/crates/`, and m5a fixture/package precedents.

Round 6 closes the remaining round-5 blocker and the two majors/nit without introducing new blocking mechanics. The `rafaello-fetch` package manifest now uses the live `[provides.tool.<tool>]` shape, logging moved into the library helper so handler tests cover it, the c08 compile-fence relaxation is explicit, and the copied mailcat-style bin template has the missing imports.

## Round-5 verification table

| r5 finding | status | verification |
|---|---|---|
| B1 c20 `rafaello-fetch/rafaello.toml` used lock-side `bindings.tool_meta` and would not parse as a package manifest | closed | c20 now gives a concrete package manifest with `[provides] tools = ["web-fetch"]` and `[provides.tool.web-fetch] sinks/grant_match/always_confirm`; it explicitly says `bindings.tool_meta.*` belongs only in the lock entry (commits.md:2466-2509). c22 says the copied fixture manifest uses the same `[provides.tool.web-fetch]` package shape (commits.md:2906-2910). |
| M1 c21 log tests did not match where logging was called | closed | c21 now says `handle_web_fetch` calls `maybe_write_invocation_log(url)` internally, and the bin no longer calls the log helper directly (commits.md:2584-2616, 2705-2709). Handler-only tests now legitimately prove logging. |
| M2 c08 silently dropped compile-fence guard | closed | c08 now has an explicit “Compile-fence policy relaxation” paragraph explaining the deliberate removal of the `trybuild` production-absence test and the reliance on the cfg/documentation contract (commits.md:1168-1183). |
| N1 c21 copied-mailcat template omitted imports needed by `OneShotConnector` tail | closed | c21’s template now imports `async_trait`, `fittings_core::{error::FittingsError, transport::Connector}`, and `tokio::sync::{broadcast, Mutex}` (commits.md:2656-2672). |

## Final pass

No blockers found.

Checks performed:

- **Manifest shape:** c20’s `rafaello-fetch` manifest now matches the live/m5a package-manifest model: package tool metadata under `[provides.tool.web-fetch]`; lock projection remains `bindings.tool_meta.web-fetch` in c22. This satisfies live `Manifest` / `Provides` parsing and keeps c22’s lock semantics intact.
- **Bus-client shape:** c21 retains the round-5 mailcat-style runtime path: `RFL_BUS_FD` adoption, `OneShotConnector`, `Client::connect`, `subscribe_notifications`, `notifications.recv()`, ACL-driven topic filtering, and `peer.notify("bus.publish", ...)`; no reintroduction of `Client::new`, `bus.subscribe`, or `client.recv`.
- **Fixture env vars:** c21 consistently documents unconditional env-var reads for `RFL_FETCH_TEST_LOG_PATH` and `RFL_FETCH_TEST_TAINT_OVERRIDE`, with no spawned-binary feature wiring dependency.
- **PT1 violation path:** c22 keeps the exact-URL grant template for `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE`, matching live structural-subset semantics.
- **No round-6 regression spotted** in the touched areas (c08/c20/c21/c22) or in the cross-check/sizing tail.

## Convergence call

Converged. The plan is ready for owner ratification from the pi review perspective.