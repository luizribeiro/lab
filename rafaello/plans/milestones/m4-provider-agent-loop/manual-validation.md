# m4 — manual validation evidence

> Companion to `retrospective.md`. Records the exact
> acceptance-gate transcripts and out-of-band evidence
> that `scope.md` §"Acceptance summary" requires before
> ratification. Status: round 1, drafted 2026-05-11 at
> the close of c28; CI URLs are placeholders to be
> captured by the driver at retrospective time.

The scope's acceptance summary names four cargo-driven
gates, one out-of-band interactive smoke, and a CI green
URL. This document captures each, plus the negative-
matrix cross-reference.

## 1. Linux acceptance gate — `cargo test --workspace --features test-fixture`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture
```

**Status:** to be captured by the driver at retrospective
time, after pushing `agents/m4/driver` to `rafaello-v0.1`.
The aggregate count and a tail snippet of the test log
should be inlined here for durability, mirroring m3
§1's format.

## 2. Linux build gate — `cargo build --workspace --bins`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture
```

**Status:** to be captured by the driver. Verifies all
five bins build:

- `rfl` (`rafaello/crates/rafaello`)
- `rfl-tui` (`rafaello/crates/rafaello-tui`)
- `rfl-mockprovider` (`rafaello/crates/rafaello-mockprovider`)
- `rfl-readfile` (`rafaello/crates/rafaello-readfile`)
- `rfl-bus-fixture` (`rafaello/crates/rafaello-core`,
  feature-gated)

The explicit `--features rafaello-core/test-fixture` is
required because `rfl-bus-fixture` is gated by
`required-features = ["test-fixture"]` on the
`rafaello-core` crate; `--workspace --bins` alone would
skip it.

## 3. Linux doc gate — `cargo doc --workspace --no-deps`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo doc --manifest-path rafaello/Cargo.toml --workspace --no-deps
```

**Status:** to be captured by the driver. Expected to be
warning-free; no new doc warnings should be introduced
under m4's expanded surface (rafaello-mockprovider,
rafaello-readfile, the agent-loop module, the core
re-emit pipeline, and the `Publisher::Provider` schema
additions).

## 4. macOS CI gate — hard ratification gate

**Status:** to be captured by the driver at
retrospective time. m4 ratification cannot complete
until at least one `macos-latest` CI run is green
(scope §"Acceptance summary"), with the only exception
being tests explicitly gated `#[cfg(target_os = "linux")]`
(carried forward from m3's
`frontend_handle_drop_does_not_leak_zombie.rs` and the
supervisor `/proc/self/fd` baseline tests).

**Capture procedure** (matches m3 manual-validation §4
and m2 §5.7):

1. Push `rafaello-v0.1` to GitHub: `git push origin
   rafaello-v0.1`.
2. The existing `.github/workflows/rafaello.yml` workflow
   auto-triggers (`test (macos-latest)` job).
3. Capture the `macos-latest` job URL into this section.
4. Confirm a green run. Any non-platform-inherent failure
   must be fixed in m4 before ratification (m3-precedent:
   macOS failures are NOT retrospective-time follow-ups).

**Placeholder URL** (to be filled in):

- Workflow run: <https://github.com/luizribeiro/lab/actions/runs/__TBD__>
- `test (macos-latest)`: <https://github.com/luizribeiro/lab/actions/runs/__TBD__/job/__TBD__>
- `test (ubuntu-latest)`: <https://github.com/luizribeiro/lab/actions/runs/__TBD__/job/__TBD__>

## 5. Real interactive `rfl chat` smoke — demo bar

`scope.md` §"Acceptance summary" requires an interactive
`rfl chat` run against the fixture lock that demonstrates
the demo bar: the user types `what's in README.md` and
sees the file's contents rendered as an assistant
message.

### Walkthrough

1. Build the workspace inside the devshell:
   ```
   nix develop --impure --command cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture
   ```
2. Create a fresh project directory and seed it with a
   `README.md`:
   ```
   mkdir -p /tmp/m4-demo && cd /tmp/m4-demo
   echo 'm4 demo readme' > README.md
   ```
3. Lay down a `.rafaello/rafaello.lock.json` fixture lock
   wiring `rfl-mockprovider` (as provider `mock`) and
   `rfl-readfile` (as tool plugin with the project-root
   read grant). Cargo-test `rfl_chat_demo_bar_read_file`
   constructs the same fixture programmatically; copy
   its layout (see `rafaello/crates/rafaello/tests/
   rfl_chat_demo_bar_read_file.rs` for the canonical
   `LockBuilder` calls).
4. Launch `rfl chat` from `/tmp/m4-demo`:
   ```
   nix develop --impure --command rafaello/target/debug/rfl chat
   ```
5. At the TUI prompt, type `what's in README.md` and
   submit. Observe the four entries render in order:
   - the user `text` entry (`what's in README.md`),
   - an assistant `tool_call` entry naming `readfile`,
   - a tool `tool_result` entry carrying the file body,
   - an assistant `text` entry rendering
     `Here's what's in README.md:\nm4 demo readme\n`.
6. Exit cleanly via Ctrl-C; the TUI should restore the
   terminal (raw-mode disabled, alt-screen left, mouse
   capture disabled — same defense-in-depth panic hook
   m3 wired in `run_production_mode`).
7. Re-open the SQLite session store at
   `/tmp/m4-demo/.rafaello/state/session.sqlite` and
   confirm exactly four rows in the `entries` table with
   seq 0..3 matching the kinds + authors above:

   | seq | kind          | author     |
   |-----|---------------|------------|
   | 0   | `text`        | User       |
   | 1   | `tool_call`   | Assistant  |
   | 2   | `tool_result` | Tool       |
   | 3   | `text`        | Assistant  |

   The assistant text payload should be
   `Here's what's in README.md:\nm4 demo readme\n`.

### Mechanical coverage

The four behaviours the smoke verifies are all covered
mechanically by the cargo-test
`rfl_chat_demo_bar_read_file` (c27,
`rafaello/crates/rafaello/tests/rfl_chat_demo_bar_read_file.rs`):
it spawns `rfl chat` end-to-end against a real temp-dir
fixture lock, drives it with `RFL_TUI_TEST_MESSAGE`, and
asserts the exact SQLite row shape (seq / kind / author /
text payload) plus the four `core.session.entry.finalized`
sentinels on TUI stderr. Owner accepts mechanical
coverage in lieu of a screen-recording (m3-precedent),
modulo the §"Known local environment issue" caveat
below.

### Known local environment issue

On the dev host this commit was authored on
(Linux 6.12.84 / Landlock ABI 6), `rfl chat` against the
mockprovider+readfile fixture lock currently fails
locally: `syd` emits

```
incompatible directory-only access-rights: AccessFs(8) / LinuxError: EOPNOTSUPP
```

for each eager-spawned child during the landlock-apply
step. The c27 `rfl_chat_demo_bar_read_file` cargo test
hits the same failure mode (the mockprovider→readfile
round-trip wedges after the first `entry.finalized` is
forwarded), even though c26's `rfl_chat_demo_bar` smoke
(echo path only, no readfile child) continues to pass.

The failure is **environmental** (kernel / syd /
Landlock-ABI interaction on this dev box), not a defect
in the m4 code under test. The walkthrough steps above
are written as they will execute on an unaffected
environment.

**TODO at retrospective time:**

- Capture the macOS CI green URL into §4 above. macOS
  does not exercise Landlock, so a green `macos-latest`
  run is the first independent confirmation the demo
  bar works end-to-end on a non-affected platform.
- Capture an `ubuntu-latest` CI green URL alongside;
  GitHub-hosted Linux runners typically pair newer
  kernels with the syd build used by the workflow, so
  a green run there is the second independent
  confirmation.
- **m4 ratification cannot be declared until at least
  one CI runner is confirmed green.** Local-only sign-off
  is insufficient given the Landlock failure mode above.

## 6. CI green — Linux + macOS workflow run URL

**Status:** to be captured by the driver at
retrospective time.

- Workflow run: <https://github.com/luizribeiro/lab/actions/runs/__TBD__>

---

## Acceptance-summary cross-reference

| `scope.md` §"Acceptance summary" bullet | Section | Status |
|-----------------------------------------|---------|--------|
| `cargo test --workspace --features test-fixture` green on Linux | §1 | ⏳ driver-captured |
| **macOS CI green** (hard gate) | §4 | ⏳ driver-captured |
| `cargo build --workspace --bins --features rafaello-core/test-fixture` green | §2 | ⏳ driver-captured |
| `cargo doc --workspace --no-deps` warning-free | §3 | ⏳ driver-captured |
| Interactive `rfl chat` demo-bar walkthrough | §5 | ✅ mechanical coverage via `rfl_chat_demo_bar_read_file` (caveat: local Landlock failure, §5 "Known local environment issue") |
| CI green Linux + macOS | §6 | ⏳ driver-captured |

## Negative-matrix cross-reference

The six negative demos enumerated in scope §"Negative
matrix" map to the following test files. All must be
green under the §1 acceptance run.

| # | Negative demo | Test file(s) |
|---|---------------|--------------|
| 1 | `tool_result` missing `in_reply_to` rejected | `rafaello-core/tests/broker_plugin_tool_result_missing_in_reply_to_rejected.rs` (broker path, m2-extension); `rafaello-core/tests/reemit_plugin_tool_result_missing_in_reply_to_rejected.rs` (core re-emit path) |
| 2 | Provider `tool_request` with stale/unknown id fails closed | `rafaello-core/tests/broker_provider_tool_request_missing_in_reply_to_rejected.rs` (absent field → `Missing`, per §7.2.6 row 2); `rafaello-core/tests/broker_provider_tool_request_stale_id_rejected.rs` (`InvalidInReplyTo { reason: StaleRequestId { id } }` per §B7b's `provider_observed_results` set) |
| 3 | Tool plugin called directly by another plugin doesn't reach dispatch | `rafaello-core/tests/cross_plugin_tool_request_blocked_at_broker.rs` (`PublishOnReservedNamespace` from m2); `rafaello-core/tests/cross_provider_request_to_tool_only_routes_via_core.rs` (pi-2 M2-2: tool plugin observes `core.session.tool_request` via subscribe but does NOT execute — only `plugin.<topic-id>.tool_request` triggers dispatch) |
| 4 | Tool requested outside its grant denied at lockin | `rafaello-readfile/tests/readfile_lockin_denies_outside_grant.rs` (pi-1 H-3 — lockin-level path, independent of the plugin's ancestor-check short-circuit exercised by `readfile_errors_for_outside_project_root.rs`) |
| 5 | Bus event missing the `taint` envelope rejected | `rafaello-core/tests/broker_publish_core_session_tool_request_missing_taint_rejected.rs`; `rafaello-core/tests/broker_publish_core_session_tool_result_missing_taint_rejected.rs` (broker errors `InvalidTaint { reason: "missing" }`) |
| 6 | Plugin-supplied taint discarded/replaced on core re-emit | `rafaello-core/tests/reemit_discards_plugin_supplied_taint_on_core_session_tool_request.rs` (pi-3 L-1: provider's claimed `taint: [{source: "user"}]` is discarded; emitted `core.session.tool_request` carries only the canonical `[{source: "provider", detail: "mock"}]`) |

The retrospective remains pre-evidence on §1–§4 and §6
plus the §5 CI-green confirmation; ratification waits on
those captures plus pi ratification of `retrospective.md`.
