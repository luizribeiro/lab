# m5b commits.md round-5 pi review

> Verdict: blocking.
> Counts: B/1 M/2 N/1

Reviewed round-5 `commits.md` at worktree tip `f4f7512` against `commits-pi-review-4.md`, the m5b scope, m5a fixture precedent, and live source under `rafaello/crates/`.

Round 5 closes the round-4 mechanics that were under active review, but the final pass found one older package-manifest blocker in the new `rafaello-fetch` scaffold: c20 still writes lock-side `bindings.tool_meta` into `rafaello.toml`. Live manifest parsing rejects unknown top-level `bindings`, and m5a mailcat’s accepted shape uses `[provides.tool.<tool>]` for `sinks` / `grant_match` / `always_confirm`.

## Round-4 verification table

| r4 finding | status | verification |
|---|---|---|
| B1 c21 still used non-live client APIs (`Client::new`, `bus.subscribe`, `client.recv`) | closed | c21 now says to copy live mailcat’s bus-client shape: `parse_bus_fd` → `adopt_bus_fd` → `OneShotConnector` → `Client::connect` → `subscribe_notifications()` → `notifications.recv()`; and explicitly forbids `Client::new`, `bus.subscribe`, and `client.recv` (commits.md:2560-2645). c20 adds `ulid` for the fresh JSON-RPC id (commits.md:2388-2391). |
| B2 log/override code was cfg-gated but spawned binaries had no feature wiring | closed | c21 drops the cfg gates and documents the env vars as unconditionally compiled test-fixture escape hatches that no-op when unset (commits.md:2508-2559). c20 removes the `test-fixture` feature (commits.md:2392-2397), and the cross-check reflects the new contract (commits.md:3526-3545). |
| B3 c22 grant template used `{"url":""}` and would not match live structural grants | closed | c22 now uses the exact URL template `{"url":"https://content.example.com/page"}` and cites live exact-value subset semantics (commits.md:2889-2912). |
| M1 c15 still allowed the helper signature to drift | closed | c15 now says the signature is authoritative and removes the “agent may adjust” hedge; c17 consumes only the return value (commits.md:1876-1917). |
| M2 c21 compile-fence tested private cfg-gated functions | closed | c21 removes the compile-fence because the functions are no longer cfg-gated/private-for-absence purposes (commits.md:2715-2718). |
| N1 commit-hash drift note | closed | The status banner adds a hash-stability note and avoids embedding round commit hashes in the artifact text (commits.md:15-21). |

## Blocking findings

### B1 — c20’s `rafaello-fetch/rafaello.toml` uses lock-side `bindings.tool_meta`, which live manifest parsing rejects

Anchor: c20 `rafaello-fetch` manifest.

c20 specifies the package manifest as:

```toml
[provides]
tools = ["web-fetch"]
...
[bindings.tool_meta.web-fetch]
sinks = ["network"]
grant_match = "schemas/web-fetch-grant.json"
always_confirm = false
```

(commits.md:2308-2325). `bindings.tool_meta` is a lock-entry table, not a package-manifest table. Live `Manifest` is `#[serde(deny_unknown_fields)]` and has top-level `provides`, `bus`, `capabilities`, `load`, and `renderers`, but no top-level `bindings` (live `manifest/top_level.rs`:21-44). Live tool metadata in manifests lives under `provides.tool`: `ToolMetaManifest { sinks, grant_match, always_confirm }` (live `manifest/provides.rs`:22-38). The m5a mailcat fixture uses the same shape:

```toml
[provides]
tools = ["send-mail"]

[provides.tool.send-mail]
sinks = ["mail"]
always_confirm = false
grant_match = "schemas/send-mail-grant.json"
```

(live `crates/rafaello-mailcat/rafaello.toml`:7-14; m5a commits.md:2005-2015).

As written, c20’s own `rafaello_fetch_manifest_compiles.rs` acceptance (`manifest::parse` + `manifest::validate_with_package`) will fail before validation because `bindings` is an unknown manifest field. It also means the c22 fixture tree “same content as c20” inherits the invalid manifest.

Smallest fix: change c20’s package manifest and c22’s copied fixture manifest to:

```toml
[provides]
tools = ["web-fetch"]

[provides.tool.web-fetch]
sinks = ["network"]
grant_match = "schemas/web-fetch-grant.json"
always_confirm = false
```

Keep `bindings.tool_meta.web-fetch` only in the lock entry.

## Major findings

### M1 — c21’s log unit-test acceptance no longer matches where logging is called

c21 makes `maybe_write_invocation_log(url)` a separate helper and says the bin `run_loop` calls it after extracting the URL (commits.md:2511-2524, 2615-2617). But the acceptance test `rafaello_fetch_writes_invocation_log_when_log_path_set.rs` still says to “invoke the handler twice” and assert the log contains two lines (commits.md:2676-2682). The described `handle_web_fetch(args)` path does not itself call `maybe_write_invocation_log`.

Fix either side: have `handle_web_fetch` call the log helper (so handler unit tests prove logging), or rewrite the acceptance to exercise the bin/run-loop publish path or call `maybe_write_invocation_log` explicitly.

### M2 — c08 silently drops a previously accepted compile-fence guard

Round 5 removes c08’s `trybuild` production-absence test for `Broker::install_publish_test_hook` (commits.md:1116-1134). The method is public when compiled (`pub fn install_publish_test_hook`, commits.md:1043-1049), so an external compile-fail fixture can still be meaningful if built without `test-fixture`. If the team intentionally accepts “cfg is self-documenting” here, this is a policy relaxation from earlier rounds and should be called out in the convergence criteria, not hidden as a B2 ripple.

## Nit findings

### N1 — c21’s structural template omits imports required by the copied mailcat tail

The c21 template imports `tokio::sync::broadcast` but says `OneShotConnector` is copied from live mailcat (commits.md:2570-2606). That copied tail also needs `tokio::sync::Mutex` and `fittings_core::error::FittingsError` (live `rfl_mailcat.rs`:17, 19, 119-143). Since the row says “copied verbatim,” the implementer can infer this, but adding the imports to the template would avoid a trivial compile stumble.

## Convergence call

Not converged. The round-4 findings are resolved, but the final pass blocker is concrete: the new fetch package manifest will not parse. After changing package manifests from `bindings.tool_meta` to `provides.tool`, the plan should be very close to convergence.