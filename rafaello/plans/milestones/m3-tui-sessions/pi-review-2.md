Round-2 review of `rafaello/plans/milestones/m3-tui-sessions/scope.md` at `39defb7`.

Verdict: **not quite ready for ratification**. Round 2 fixed most round-1 issues, but I still see several blocking spec inconsistencies / implementation gaps.

## Blocking / high findings

1. **Frontend connection service is underspecified and currently contradictory**
   - §F3 says reuse m2 `BusPublishService` for the frontend connection (`lines 315–318`).
   - §B2 adds `Broker::handle_frontend_publish` (`lines 436–438`).
   - Existing m2 `BusPublishService` is plugin-specific: it stores a `CanonicalId` and calls `handle_plugin_publish`.
   - §C2 step 7 also requires an extra readiness service (`lines 932–937`), but §F1/F3 expose no production API for composing extra services into `FrontendSupervisor`.
   - Fix: specify a principal-aware connection service, or a dedicated `FrontendBusPublishService`, plus a built-in frontend readiness handler/channel.

2. **Replay cannot publish the canonical `seq`**
   - `append_entry` returns `seq` (`lines 496–498`), and fresh `finalized` envelopes carry `seq` (`lines 515–516`, plus `lines 657–662`).
   - But `load_entries()` returns only `Vec<Entry>` (`lines 499–501`), and `seq` is explicitly not in `EntryMetadata` (`lines 657–666`).
   - Therefore `replay_history()` cannot reconstruct the same envelope shape for replay.
   - Fix: return `Vec<StoredEntry { seq, entry }>` or equivalent.

3. **Fixture-entry path contradiction remains**
   - Goal still says fixtures inject directly through broker `publish_core` (`lines 73–77`).
   - §S and §C say fixtures must go through `SessionController::finalize_entry` (`lines 474–478`, `948–954`).
   - Fix the Goal text; direct `publish_core` would bypass persistence/rendering.

4. **TUI test-placement rule is still violated**
   - The doc claims core tests do not need `rfl-tui` (`lines 1092–1098`) and TUI-spawning helpers belong in `rafaello-tui` because of `CARGO_BIN_EXE_rfl-tui` (`lines 1266–1271`).
   - But `frontend_handle_wait_resolves_on_child_exit.rs` remains under `rafaello-core/tests/` and spawns `rfl-tui` via an unspecified build-script path (`lines 1168–1177`).
   - Fix: move it to `rafaello-tui`, or use a core-local stub child so the test no longer depends on `rfl-tui`.

5. **Session lock should be acquired before touching SQLite**
   - §S1 says `open()` opens/creates SQLite, runs PRAGMAs, creates tables, then acquires flock (`lines 489–495`).
   - That means a second process can still open/mutate SQLite before being rejected by the lock.
   - Fix: create state dir, acquire `session.lock`, write holder pid, then open/configure SQLite.

## Medium findings

6. **Readiness handshake lacks named tests and a clean method contract**
   - §C2 introduces `core.lifecycle.frontend_ready` as a peer notification (`lines 932–937`), but this looks like a core bus topic despite being frontend-originated and not broker-authorized.
   - Add explicit tests: ready timeout errors, replay withheld until ready, ready sent after handler registration.
   - Consider naming it as an RPC method outside bus-topic namespace, e.g. `frontend.ready`.

7. **`Raw::Html` downgrade is impossible with current capabilities**
   - §E2 says TUI accepts only `Ansi`/`Plain`; `Html` triggers downgrade (`lines 685–687`).
   - §R3/R4 only downgrade based on `Capabilities::nodes`; `Raw` is a supported node and capabilities have no `raw_formats` field (`lines 773–806`).
   - Fix: add raw-format capabilities or state that `Raw(Html)` is always downgraded by policy.

Overall: **close, but needs another small edit pass** before round-2 convergence.
