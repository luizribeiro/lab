# m6.1 — v0.1.1 cold-start patches — scope

> **Status:** round 1 — claude-authored 2026-05-12, awaiting pi
> round 1. First post-v1 patch milestone. Forked off `main` at
> `1e0ba53` (the m6 RATIFIED commit declaring v1 demo-ready).
> Driver-preflight `821e5c0`.

## Goal

Close the two cold-start defects the owner hit running the
just-shipped `rfl init && rfl chat` flow against the m6
RATIFIED build, plus the regression tests that re-walking the
same cold-start path would have caught.

**Owner directive (verbatim from driver prompt):** "two defects
in, two defects fixed, regression tests added so they can't
slip through again. No new features." Sized 5–10 commits.

**Defects in scope:**

1. **D1.** `rfl init` materialises the m4-c20 placeholder shim
   (`#!/bin/sh\nexec "$@"`) at the plugin's `entry` path, so
   syd's spawn of `bin/rfl-openai` falls back to `/bin/sh` (not
   in `exec_dirs` grant) → `Permission denied`. Repro: cold
   `rfl init && cargo run --bin rfl -- chat` from a fresh
   `~/lab/rafaello` checkout.
2. **D2.** `rfl chat`'s Ctrl-C does not terminate the TUI. Root
   cause: `rfl-tui` enables crossterm raw mode, so the kernel
   TTY discipline does not generate SIGINT; the Ctrl-C byte
   arrives as a `KeyModifiers::CONTROL + KeyCode::Char('c')`
   key event but `handle_terminal_event` does not match it.
   The parent's `tokio::signal::ctrl_c()` listener is
   correctly installed but is never reached.

**Not in scope** (deferred to a future milestone or filed as
out-of-band): everything else. No new commands, no docs pass,
no audit-CLI extensions, no syd-pty layering revisits, no
overview/decisions structural changes beyond a single append
row covering the materialisation-time runtime-binary contract.

## Inputs

### From the plans tree

- `milestones/README.md` — milestone-roadmap context (m6 is
  the last RATIFIED row; m6.1 is a patch *of* m6, not a new
  roadmap row).
- `milestones/m6.1-cold-start-patches/driver-preflight.md`
  (`821e5c0`).
- `milestones/m6-polish-release/retrospective.md` (especially
  §"Phase K" — the cK1..cK6 production-TUI tmux-driven
  integration-test pattern that D2's regression test mirrors).
- `overview.md` §8.1 (bundled provider plugin), §15.1
  (manifest), §16 (v1 scope).
- `decisions.md` rows 38 (bundled `rfl-openai`), 59–68 (m6
  ratification batch — esp. the PP1 plugin-tree contract).
- `glossary.md` — PP1, canonical id, topic id.

### From the codebase

- `rafaello/crates/rafaello/src/init.rs` (c21).
- `rafaello/crates/rafaello/src/bundled.rs` (c21).
- `rafaello/crates/rafaello/src/pp1.rs` (m6 PP1).
- `rafaello/crates/rafaello-openai/bin/rfl-openai` (the
  20-byte shim — m4 c20).
- `rafaello/crates/rafaello/tests/common/m4_install.rs`
  (`install_demo_layout` — the test helper that bypasses the
  shim by copying `workspace_bin("rfl-mockprovider")` over
  it; the routing-around that masked D1).
- `rafaello/crates/rafaello/tests/common/workspace_bin_path.rs`
  (`workspace_bin` builder helper — reuse for D1's regression
  test).
- `rafaello/crates/rafaello-tui/src/bin/rfl_tui.rs` lines
  320–479 (`handle_terminal_event`).
- `rafaello/crates/rafaello/src/lib.rs` lines 540–760
  (`run_chat_after_spawns` / `run_post_ready` — the parent's
  ctrl_c listener; **unchanged** by this milestone, the fix is
  in the child).
- `rafaello/crates/rafaello/tests/rfl_chat_eager_spawns_*.rs`
  + `rfl_chat_constructs_gate_before_provider_spawn.rs` —
  existing patterns for "kill child with SIGINT after sentinel"
  (these send SIGINT to the parent directly via `nix::signal`;
  they do **not** exercise the raw-mode-Ctrl-C path).

## In scope

### §A — D1: shim → real-binary at materialisation time

**A1.** New helper `rafaello::bundled::resolve_runtime_binary(name)
-> Result<PathBuf, BundledError>`. Resolution order, mirroring
the existing `resolve_plugin_dir`:

1. `RFL_BUNDLED_BIN_<NAME_UPPER>` env var (test override;
   explicit per CLAUDE.md "errs-toward-explicit-config").
   `<NAME_UPPER>` is the plugin name with hyphens → underscores
   and uppercased (e.g. `openai` → `RFL_BUNDLED_BIN_OPENAI`).
   Value is the absolute path to the binary; resolver validates
   it is a regular file and executable, else
   `BundledError::NotFound`.
2. **Release layout.** `<rfl-exe-parent>/rfl-<name>` (matches
   the m6 c-PP1 contract: top-level `<release-prefix>/bin/`
   carries `rfl`, `rfl-tui`, and each `rfl-<plugin>` runtime
   binary).
3. **Dev fallback.** Walk up from `<rfl-exe-parent>` looking
   for a workspace root (`Cargo.toml` containing `[workspace]`);
   if found, return `<workspace>/target/<profile>/rfl-<name>`
   where `<profile>` is `debug` in `cfg!(debug_assertions)`
   else `release`. (Same shape as
   `tests/common/workspace_bin_path.rs` — but inside the prod
   `bundled` module, since prod-`rfl` run via `cargo run` is
   the *exact* path that hit the bug.)
4. None of the above → `BundledError::NotFound { name }` with
   the message `"no rfl-<name> runtime binary discoverable
   (tried env <NAME>_BIN_OPENAI, <rfl-exe-parent>/rfl-<name>,
   workspace target/<profile>/rfl-<name>)"`.

**A2.** In `init::run`, after `pp1::materialise(...)` returns
`target_dir`:

1. Resolve the runtime binary via
   `bundled::resolve_runtime_binary("openai")`.
2. Compute `entry_absolute = target_dir.join(manifest.entry.as_str())`.
   This file already exists (the shim copied by PP1).
3. `fs::copy(runtime, &entry_absolute)` — overwrite the shim
   with the real binary bytes.
4. `fs::set_permissions(&entry_absolute, 0o755)` — preserve
   exec bit explicitly (the shim was 0o755; PP1's copy already
   preserves mode, but the resolver may return a binary with
   non-exec mode in unusual environments — be defensive).
5. **Recompute the content digest after the swap**, before
   sealing the `PluginEntry`. The lock's `digest` field must
   reflect the real binary, not the shim. Today
   `digest::content_digest(&target_dir)` is called *after*
   `pp1::materialise` and *before* the swap; round 1 moves
   the digest call to after step (4).

**A3.** Failure mode: if `resolve_runtime_binary` returns
`NotFound`, `rfl init` exits non-zero with the resolver's
error message verbatim to stderr. **No silent shim-shipping.**
The materialised package dir is cleaned up on this failure
(remove `target_dir`) so a retry against a fixed environment
starts clean. The `rafaello.lock` is not written on failure.

**A4. Why not "skip the shim entirely in `rafaello-openai/`"?**
Considered + rejected. `manifest::validate_with_package` (a
load-bearing m1 invariant) requires `entry` to resolve to a
regular file inside `package_dir` at *validation* time —
including at `cargo test`-time and at manifest-author lint
time, well before any `rfl init` materialisation has occurred.
Removing the shim from the source tree would break m4+m5+m6
in-tree fixture validation. The right layer for the swap is
the materialisation step (`rfl init`), where we have the
project root and the resolver in hand. The shim stays
in-tree as the manifest-validator placeholder it was always
intended to be.

**A5. Why overwrite in-place rather than redirect
`entry_absolute` outside `package_dir`?** The lock's
`PluginEntry.entry` is a `SafePath` constrained to live under
`package_dir` by the m1 PP1 containment invariant
(`compile::resolve_entry`'s `EntryEscape` check). Redirecting
elsewhere would either break PP1 or require a symlink that
also breaks PP1 (resolved symlinks must canonicalise back
inside `package_dir`). Overwriting the file in place keeps the
invariant trivially satisfied.

**A6. Why a new `RFL_BUNDLED_BIN_<NAME>` env var instead of
extending `RFL_BUNDLED_PLUGINS_DIR` semantics?** Owner-ratified
2026-05-12: explicit > implicit (CLAUDE.md DRY + explicit-config
principle). `RFL_BUNDLED_PLUGINS_DIR` is the *source-tree*
override (it points at where to read the manifest/openrpc/shim
from); `RFL_BUNDLED_BIN_<NAME>` is the *runtime-binary*
override (it points at the executable that replaces the shim).
Conflating them would force every test that sets the source
tree to also rebuild a real binary into it, which is
unnecessary friction.

### §B — D2: Ctrl-C in `rfl-tui` → clean Quit

**B1.** In `rfl_tui.rs::handle_terminal_event`, add a Ctrl-C
guard *before* the mode dispatch:

```rust
if key.modifiers.contains(KeyModifiers::CONTROL)
    && key.code == KeyCode::Char('c')
{
    return EventOutcome::Quit;
}
```

This fires regardless of `InputMode` (Normal /
ConfirmOverlay), so Ctrl-C while the confirm modal is up also
quits. The existing `EventOutcome::Quit` path is already
correct: `ui_loop` returns Ok → `restore_terminal()` drops
the alternate screen + raw mode → child exits 0 → parent's
`handle.wait()` resolves → `run_post_ready`'s shutdown
sequence runs.

**B2. Why not also wire Ctrl-D?** Deferred. Owner directive
is "two defects fixed", and Ctrl-D-on-empty-input vs
Ctrl-D-as-EOF is its own UX decision worth a separate scoping
discussion. m6.1 fixes only the gesture the owner hit.

**B3. Why not also fix the parent's signal-forwarding?** The
parent's `tokio::signal::ctrl_c()` listeners are already
correct. The bug is that no SIGINT is generated in the first
place (raw mode disables ISIG on the controlling TTY). Fixing
the child is sufficient; touching the parent's signal layer
is out of scope and would risk regressing the existing
test-driven `nix::signal::kill(SIGINT)` paths in the
`rfl_chat_eager_spawns_*.rs` suite.

**B4. Symmetric application across `InputMode` variants.**
The Ctrl-C guard sits before the `match mode { Normal { … },
ConfirmOverlay { … } }` dispatch, so it pre-empts both. In
particular: pressing Ctrl-C while a sink-confirm modal is
open immediately exits the TUI, which (a) drops the modal,
(b) implicitly denies the in-flight confirmation (the
confirmation reply task in the parent observes the child's
exit and treats unanswered confirmations as "session torn
down"), (c) is the gesture least likely to surprise a user
who is panicking because the assistant just asked to do
something dangerous. Acceptance test asserts the Ctrl-C
key-event handler returns `Quit` in both modes (table-driven
unit test).

### §C — Regression coverage

**C1. (D1 unit/acceptance, fast-cycle.)** New test
`rafaello/crates/rafaello/tests/rfl_init_materialises_real_runtime_binary.rs`:

- Build a sentinel "fake `rfl-openai`" binary via the
  `workspace_bin("rfl-openai-stub")` helper (existing crate
  `rafaello-openai-stub` — already in the workspace, already
  builds, already used by §F2 m6 tests).
- Set `RFL_BUNDLED_BIN_OPENAI` to that path.
- Set `RFL_BUNDLED_PLUGINS_DIR` to a temp directory mirroring
  `rafaello/crates/rafaello-openai/` (manifest + openrpc +
  the in-tree shim — exact fixture vintage).
- Invoke `rafaello::init::run(InitArgs { yes: true, force:
  true, project_root: Some(tmp) })`.
- Assert the file at `tmp/.rafaello/plugins/<topic>/bin/rfl-openai`
  is byte-identical to the resolved `rfl-openai-stub` binary
  (compare by SHA-256 of the file content).
- Assert it is **not** the 20-byte shim (length > 20 and the
  shebang `exec "$@"` literal is absent — defensive).
- Assert the lock's `PluginEntry.digest` matches a fresh
  `content_digest(target_dir)` computed *after* the swap.

**C2. (D1 cold-start regression, subprocess.)** New test
`rafaello/crates/rafaello/tests/rfl_init_runtime_binary_outside_cargo_env.rs`:

- Build `rfl` via `workspace_bin("rfl")`.
- Spawn it as `std::process::Command::new(rfl_path).arg("init")
  .arg("--yes").arg("--project-root").arg(tmp)`,
  **`.env_clear()`** followed by an explicit allowlist
  (`PATH`, `HOME`, the two `RFL_BUNDLED_*` overrides pointing
  at the workspace-built tree, and nothing else). The
  `.env_clear()` is the critical step that reproduces the
  bug — without it, `CARGO_BIN_EXE_*` from the parent
  `cargo test` invocation leaks through and the bug self-heals.
- Assert exit status 0.
- Assert the materialised entry file is the real binary
  (size > 1024 bytes — heuristic, platform-neutral; magic-byte
  ELF/Mach-O detection is more rigorous but adds platform
  code and the size heuristic is sufficient when paired with
  C1's exact-bytes assertion).
- **Stop here** — does not invoke `rfl chat`. Lock-correctness
  is what C2 proves; the full spawn path is exercised
  indirectly by C4. (Owner-ratified 2026-05-12.)

**C3. (D2 unit, fast-cycle.)** Extend the existing
`rfl_tui.rs` test module (lines 595+) with a table-driven test
covering Ctrl-C in both `InputMode::Normal` and
`InputMode::ConfirmOverlay { … }`:

- Construct a `KeyEvent { code: KeyCode::Char('c'),
  modifiers: KeyModifiers::CONTROL, kind: Press, state: NONE }`.
- For each mode in `[Normal, ConfirmOverlay { … }]`, call
  `handle_terminal_event(Event::Key(key), &mut mode, &mut
  scroll, &mut input_buffer, 0)` and assert the result is
  `EventOutcome::Quit`.

**C4. (D2 end-to-end, tmux-driven — m6 cK5 pattern.)** New
test `rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`:

- Reuse the m6 `install_demo_layout(InstallOptions { real_binaries:
  true, .. })` helper to materialise a fully-valid lock with
  the mockprovider + readfile pair under a tmp project root.
- Spawn `rfl chat --project-root <tmp>` under a fresh tmux
  session (`rafaello-v0_1_1-c<NN>-ctrlctest`, name derived from
  test name + nonce to avoid reuse).
- `tmux-wait` on the parent-emitted sentinel
  `"rfl-chat: frontend-ready-observed"` (already written by
  `run_chat_after_spawns` step 7).
- `tmux send-keys -t <session> C-c`.
- Poll the child PID's status with a bounded timeout (5s);
  assert it exits with status 0, **not** killed by signal, and
  no zombie child remains under the tmux pane (reap check
  via `Pid::wait`).
- Tear down the tmux session in a guard so failures don't
  leak sessions.

This is the load-bearing regression anchor for D2: a unit
test of the key handler would have passed before the fix, but
only the tmux-driven test exercises the full raw-mode TTY
path that the bug lived on.

### §D — `manual-validation.md` v0.1.1 §1 appendix

**D1.** Append a `manual-validation.md` v0.1.1 §1 subsection
(under `rafaello/plans/milestones/m6.1-cold-start-patches/manual-validation.md`,
new file — mirrors m6 cK6's `section-5-phase-k/` appendix
pattern). Three captures:

1. `01-cold-init.txt` — `rfl init` against an empty project
   root from a freshly-built `rfl` (no `CARGO_BIN_EXE_*` env
   ambient). Asserted by visual inspection: lock written,
   accept prompt shown, no error.
2. `02-cold-chat.txt` — `rfl chat` against the same project,
   tmux-captured frame showing the initial TUI render and the
   provider-spawn sentinel in stderr. Bug-pre-fix: would show
   `syd: exec error: Permission denied`. Post-fix: clean.
3. `03-ctrl-c-exit.txt` — tmux capture spanning the Ctrl-C
   keystroke through the parent's exit. Bug-pre-fix: the TUI
   stays alive. Post-fix: clean exit, terminal restored.

All three captures land under `transcripts/v0_1_1/` in the
milestone dir.

## Out of scope

Explicit deferrals (owner directive: "tightly scoped"):

- **Ctrl-D / Esc as additional quit gestures.** Defer; D2
  fixes only Ctrl-C.
- **Parent-side signal forwarding refactor.** The current
  `tokio::signal::ctrl_c()` listeners are correct.
- **Refactoring `pp1::materialise` to take a runtime-binary
  resolver as a parameter.** PR-temptation; the swap is one
  helper call in `init::run` and clarity beats abstraction
  here. Revisit if `rfl install` grows the same pattern in a
  future milestone.
- **`rfl install` symmetry.** `rfl install` (m6 c-Phase-B)
  has the same shape — it `pp1::materialise`s a bundled or
  fixture tree, and *if* a future bundled tool also ships
  with a shim, it would hit the same bug. Today no shipped
  bundled tool other than `rfl-openai` exists; the m6
  `rfl-mailcat` demo tool ships its real binary directly. The
  symmetric fix (apply runtime-binary swap inside
  `pp1::materialise` itself, or as a post-materialise hook in
  `rfl install`) is **deferred** until a second bundled tool
  with a shim actually ships. Filed as a v0.1.2 follow-up
  candidate in this scope's §"Follow-ups".
- **macOS Mach-O magic-byte detection in C2.** Size heuristic
  is sufficient when paired with C1's exact-byte equality.
- **`overview.md` / `decisions.md` structural changes.**
  Permitted: one `decisions.md` append row stating the
  materialisation-time runtime-binary swap contract. Beyond
  that, no overview drift.
- **Docs pass on README / CONTRIBUTING.** m6 already shipped
  the 5-line bootstrap; v0.1.1 does not extend it.
- **`rfl-tui` modal-Esc / overlay-cancel semantics.** Out of
  scope.
- **Bundled-plugin-resolver layer rationalisation
  (`bundled.rs` vs `compile::resolve_entry`).** The new helper
  lives in `rafaello/src/bundled.rs` per the existing pattern.
  Centralisation discussion deferred — would expand the
  rafaello-core crate surface and is a v0.2 concern.

## Demo bar

Two assertions, both via `cargo test`:

1. `cargo test --manifest-path rafaello/Cargo.toml
   --workspace --features rafaello-core/test-fixture -p
   rafaello --test rfl_init_runtime_binary_outside_cargo_env`
   passes — proves D1 fix.
2. `cargo test --manifest-path rafaello/Cargo.toml
   --workspace --features rafaello-core/test-fixture -p
   rafaello --test rfl_chat_ctrl_c_quits_cleanly` passes —
   proves D2 fix.

Plus the §D `manual-validation.md` appendix transcripts on
disk in the milestone dir.

## Owner-judgment items

(Items where the owner has already weighed in get the
ratification timestamp inline; pi can still push back if it
sees a flaw.)

1. **Milestone directory name.** `m6.1-cold-start-patches`
   vs `m7-v0.1.1-cold-start-patches`. **Owner-ratified
   2026-05-12: `m6.1-...`** (patch-of-m6 semantics).
2. **C2 test depth.** Stop at lock-correctness vs extend to
   end-to-end spawn. **Owner-ratified 2026-05-12: stop at
   lock-correctness; C4 tmux Ctrl-C test covers the spawn
   path as a side effect.**
3. **`RFL_BUNDLED_BIN_<NAME>` vs extending
   `RFL_BUNDLED_PLUGINS_DIR`.** **Owner-ratified 2026-05-12:
   new env var, explicit > implicit.**
4. **Should A1's resolver also accept a fully-baked
   `.rafaello/plugins/<topic>/bin/rfl-openai` already present
   in the source dir** (i.e. if some future packager
   pre-materialises the binary into the bundled source tree,
   skip the swap)? Round-1 lean: **no**. The shim's presence
   in the source tree is a structural invariant of
   `rafaello-openai/`, and adding a "maybe-it's-already-real"
   branch obscures it. Pi can push back.
5. **Does C4's tmux session name need a per-test nonce?**
   Round-1 lean: yes (e.g. PID-suffix or `Ulid`-suffix), per
   the m1–m6 "never reuse a name" guardrail and the m6 cK5
   precedent of `rafaello-m6-cK5-<nonce>` naming. Pi can
   push back if a simpler scheme works.
6. **Does the test in C4 need to assert any audit-log
   contents** (e.g. that the Ctrl-C quit produces a graceful
   shutdown audit row)? Round-1 lean: **no**. Audit-log
   contents on graceful shutdown are not specified anywhere
   in m6's RATIFIED contract; asserting them here would be
   speculative coverage. Limit C4 to "exit status 0 + no
   zombie + terminal restored".
7. **Defensive `set_permissions(0o755)` in A2 step 4 — paranoia
   or required?** Round-1 lean: required as belt-and-braces;
   on Linux/macOS the swap-by-`fs::copy` preserves source
   mode, but the resolved binary's mode depends on how the
   workspace-target build wrote it (cargo writes 0o755). One
   line of paranoia is cheap.

## Coverage / regression-anchor list

| Test                                                           | Covers       | Layer            |
|----------------------------------------------------------------|--------------|------------------|
| `rfl_init_materialises_real_runtime_binary.rs` (C1)            | D1 §A2/§A3   | in-process       |
| `rfl_init_runtime_binary_outside_cargo_env.rs` (C2)            | D1 cold-start| subprocess       |
| `rfl_tui.rs` test module Ctrl-C cases (C3)                     | D2 §B1       | unit             |
| `rfl_chat_ctrl_c_quits_cleanly.rs` (C4)                        | D2 end-to-end| tmux-driven      |

No coverage gap is *closed* beyond the two defects; the
milestone deliberately does not expand the regression-anchor
sweep beyond D1/D2.

## Internal split (driver guidance for `commits.md`)

Proposed commit order, 6 commits:

1. **c01** — `feat(rafaello): bundled::resolve_runtime_binary
   helper`. New function in `rafaello/src/bundled.rs`. Unit
   tests in the same file (env-override hit / release-layout
   hit / workspace-target hit / not-found).
2. **c02** — `fix(rafaello): rfl init swaps shim for runtime
   binary at materialisation time`. Edits `init::run` per
   §A2; adds C1 acceptance test
   (`rfl_init_materialises_real_runtime_binary.rs`).
3. **c03** — `test(rafaello): rfl init regression — subprocess
   without CARGO_BIN_EXE_*`. Adds C2
   (`rfl_init_runtime_binary_outside_cargo_env.rs`).
4. **c04** — `fix(rafaello-tui): Ctrl-C quits TUI cleanly`.
   Edits `handle_terminal_event` per §B1; adds C3 table-driven
   unit test.
5. **c05** — `test(rafaello): tmux-driven Ctrl-C regression
   for rfl chat`. Adds C4
   (`rfl_chat_ctrl_c_quits_cleanly.rs`).
6. **c06** — `docs(rafaello-v0_1_1): manual-validation.md
   v0.1.1 appendix`. Adds §D's three captures under
   `transcripts/v0_1_1/` + the appendix file.

Drift / retrospective commits land at the retrospective
phase (one `decisions.md` row at minimum; possibly a
`glossary.md` entry if "materialisation-time runtime-binary
swap" lands as a load-bearing term).

## Follow-ups (filed, deferred)

- **v0.1.2 candidate:** symmetric fix for `rfl install` if a
  second bundled tool ever ships with a manifest-validator
  shim (move the runtime-binary swap into
  `pp1::materialise` or a shared post-materialise hook).
  Today only `rfl-openai` ships bundled with a shim; the m6
  `rfl-mailcat` demo tool ships its real binary directly, so
  no second instance exists.
- **v0.1.x candidate:** Ctrl-D / Esc / modal-cancel as
  additional quit gestures, per their own UX scoping.
- **v0.2 candidate:** `bundled.rs` resolver layer
  rationalisation — possibly merge into rafaello-core's
  `compile` module as a `resolve_runtime_binary_for(canonical,
  manifest)` API; would let the same helper serve `rfl init`,
  `rfl install`, and any future tooling.

## Acceptance summary

- D1 closed: `rfl init` materialises real runtime binary;
  cold-start `rfl chat` no longer emits `syd: exec error:
  Permission denied` on the openai plugin spawn. Asserted by
  C1 (in-process) + C2 (subprocess without `CARGO_BIN_EXE_*`
  leak).
- D2 closed: Ctrl-C in `rfl chat` exits cleanly in both
  Normal and ConfirmOverlay modes. Asserted by C3 (unit) + C4
  (tmux-driven end-to-end).
- §D `manual-validation.md` appendix on disk under
  `rafaello/plans/milestones/m6.1-cold-start-patches/transcripts/v0_1_1/`.
- One `decisions.md` row appended at retrospective time
  documenting the materialisation-time runtime-binary swap
  contract.
- No other code touched. No new commands. No overview
  drift. No regression sweep beyond the two defects.

## References

- `decisions.md` rows 38 (bundled `rfl-openai`), 59–68
  (m6 ratification batch, esp. PP1 plugin-tree contract).
- `overview.md` §8.1 (bundled provider plugin), §16 (v1
  scope).
- m6 retrospective §"Phase K" (cK1..cK6 production-TUI
  tmux-driven test precedent).
- m6 c21 (`c95f151`) — `rfl init` materialises default lock
  + PP1 bundled-plugin copy.
- m4 c20 — the manifest-validator placeholder shim
  introduced.

## Disagreements with pi (cumulative)

(None yet — pi review round 1 pending.)
