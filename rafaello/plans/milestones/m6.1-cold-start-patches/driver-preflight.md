# rafaello v0.1.1 — driver pre-flight

Driver: claude in worktree `/home/luiz/lab-wt/v0.1.1-driver` on
`agents/v0.1.1/driver`, forked off `main` at `1e0ba53` (the m6
RATIFIED commit — v1 demo-ready). Date: 2026-05-12.

## Milestone shape in one paragraph

First post-v1 patch. Two cold-start defects surfaced the moment the
owner ran the just-shipped `rfl init && rfl chat` flow against the
RATIFIED m6 build. Scope: fix both, add regression tests that
re-walking the exact same cold-start path would have caught,
**nothing else**. No new features, no surrounding polish, no scope
creep — the owner's directive explicitly bounds this at 5–10 commits.

## Proposed milestone directory name

**`m6.1-cold-start-patches`** (chosen over `m7-v0.1.1-cold-start-patches`).
Rationale:

- v1 already shipped. This is a patch *of* m6, not a brand-new
  milestone — `0.1.1` semver explicitly says "patch on 0.1.0".
- Putting it under `m6.x` keeps the milestones README roadmap clean
  (the v1 path is m0..m6; patches don't pollute the bottom-up
  sequence).
- `m7` would imply a new feature stream is in flight, which sets
  the wrong expectation for the next owner-driven roadmap row.

(Will surface this naming choice in scope.md round 1 for owner
sanity-check before pi review.)

## Defect 1 — shim-as-runtime-binary

Confirmed in code:

- `rafaello/crates/rafaello-openai/bin/rfl-openai` is the 20-byte
  shim `#!/bin/sh\nexec "$@"` (manifest-validator placeholder, m4
  c20 vintage — `manifest::validate_with_package` needs a regular
  file at the `entry` path).
- `init.rs::run` calls `pp1::materialise` which `copy_tree_dereferenced`s
  the bundled crate's tree verbatim into
  `${PROJECT_ROOT}/.rafaello/plugins/<topic>/`. The shim file is
  what ends up at `bin/rfl-openai` in the install dir.
- The lock's `PluginEntry.entry` is the relative `bin/rfl-openai`
  `SafePath`. `compile::resolve_entry` joins it against `package_dir`
  to produce `entry_absolute` — which then **is the shim**.
- When syd execs the shim, `exec "$@"` (no args) falls back to
  invoking `/bin/sh`, which is outside the sandbox's `exec_dirs`
  grant → `Permission denied` to stderr.
- The c21 acceptance tests run under `cargo test`, where
  `CARGO_BIN_EXE_rfl-openai` is set automatically — but no test
  actually exercises the materialised shim *as the spawn entry
  point* from a non-cargo-test subprocess. The
  `install_demo_layout` helper at `tests/common/m4_install.rs`
  explicitly bypasses the shim by copying the cargo-built binary
  (`workspace_bin("rfl-mockprovider")`) on top of it. So the demo
  layout used in m4 spawn tests, the manual-validation transcripts
  in m6 §5, and the c21 init test all routed around the bug.

### Initial fix shape (to refine in scope.md)

**Resolve the runtime binary at materialisation time** and write
it into the install dir at the entry path, replacing the shim.
Two layers to consider:

1. **Where to resolve.** Reuse `bundled::resolve_plugin_dir`'s
   layered approach in a new `bundled::resolve_runtime_binary(name)`:
   - `RFL_BUNDLED_BIN_<NAME>` env var (parity with `RFL_BUNDLED_PLUGINS_DIR`)
     — test override.
   - Release layout: `<rfl-exe-parent>/rfl-<name>` (Homebrew + nix
     build artefact — m6 c PP1 lays out bin/ alongside).
   - Dev fallback: `<workspace-target>/<profile>/rfl-<name>`
     (workspace_bin helper pattern).
   - None → error `"no rfl-<name> binary discoverable"` (no
     silent shim-shipping).
2. **Where to overwrite.** In `init.rs::run`, after
   `pp1::materialise`, take `entry_absolute = target_dir.join(manifest.entry)`,
   resolve the runtime binary, `fs::copy` over the shim, chmod 0o755.
   The lock's `PluginEntry.entry` already points at the right
   relative path; only the file *content* needs swapping.

Alternative considered + rejected: make `entry_absolute` in the
lock point outside `package_dir` (e.g. directly at the resolved
binary). This breaks the PP1 containment invariant
(`compile::resolve_entry` rejects entries that escape package_dir
via symlinks/relative traversal). Sticking to "materialise the
real bytes into package_dir" preserves PP1 cleanly.

### Regression-test gap (to close)

`tests/common/m4_install.rs` and `install_demo_layout` route
around this code path entirely. The c21 acceptance tests use
in-process `init::run` against fixture roots — they validate
file presence, not exec-ability. A real cold-start test needs to:

- Build `rfl` via the existing `workspace_bin` helper.
- Spawn `rfl init --yes --project-root=<tmpdir>` as a child
  process **with `CARGO_BIN_EXE_*` env vars *cleared*** (so the
  bug repros — without clearing, in-process helpers like
  `bundled::resolve_runtime_binary` might pick up cargo's env
  ambient leak through `--features test-fixture` builds).
- Read the materialised lock + the file at `entry_absolute`,
  assert it is **not** the 20-byte shim (e.g. byte-size > 1KB,
  or check magic bytes for ELF/Mach-O).
- Optionally, follow up with a `rfl chat` invocation that
  actually spawns the provider and asserts the syd permission
  error does not occur.

The "follow up with `rfl chat`" half would replicate Hard
Requirement #1 from m6 in spirit, but more rigorously. Worth
discussing in scope.md whether c02 stops at "lock points at real
binary" or extends to "spawn succeeds end-to-end" — the latter
is more expensive (full syd path + permissions plumbing) but
catches more regression surface.

## Defect 2 — Ctrl-C in `rfl chat` doesn't terminate

Code-level diagnosis (read `rfl_tui.rs` lines 320–479,
`rafaello/src/lib.rs` lines 540–760):

- Parent `rfl chat` *does* install `tokio::signal::ctrl_c()`
  handlers in both `run_chat_after_spawns` (pre-ready race) and
  `run_post_ready` (post-ready race). Those are correct.
- **The TTY layer is what's broken.** `rfl-tui` calls
  `enable_raw_mode()` + `EnterAlternateScreen` (rfl_tui.rs:326).
  In raw mode, the kernel TTY discipline does not generate SIGINT
  on Ctrl-C; instead, the byte arrives as a key event with
  `KeyModifiers::CONTROL + KeyCode::Char('c')`.
- The TUI's `handle_terminal_event` (rfl_tui.rs:404) matches `q`
  → Quit but **does not match Ctrl-C**. The keystroke is
  silently swallowed.
- Net effect: neither the parent (no SIGINT generated by TTY)
  nor the child (no key handler) responds. The user has to ^Z
  + `kill %1`.

### Initial fix shape

Wire Ctrl-C in `rfl-tui::handle_terminal_event` to
`EventOutcome::Quit`, regardless of `InputMode` (Normal /
ConfirmOverlay):

```rust
if key.modifiers.contains(KeyModifiers::CONTROL)
    && key.code == KeyCode::Char('c')
{
    return EventOutcome::Quit;
}
```

Quit triggers the existing clean-shutdown path (`ui_loop` returns
Ok → `restore_terminal()` runs → child exits → parent's
`handle.wait()` resolves → `run_post_ready` falls through to the
existing shutdown sequence).

Symmetrically: `Ctrl-D` on empty input is a common second exit
gesture; could include or defer (scope.md call). I lean toward
defer — owner directive says "tightly scoped, two defects fixed",
and Ctrl-D is its own UX decision.

### Regression-test gap (to close)

Two layers:

1. **Unit test in `rfl_tui.rs`** (cheap): inject a
   `KeyEvent { code: Char('c'), modifiers: CONTROL }` into
   `handle_terminal_event`; assert `Quit`.
2. **Tmux-driven end-to-end** (m6 cK5/cK6 pattern): launch
   `rfl chat` under tmux, `send-keys C-c`, `tmux-wait` for
   process exit with bounded timeout, assert exit status + child
   reap. m6's cK5 already established the pattern; reuse the
   helpers.

The tmux test is essential — the unit test would have passed
before the fix landed (the key handler signature was already
testable; nobody just added the assertion). Only an integration
test that drives the *full TTY raw-mode path* would catch
"keystroke arrives but never converts to action".

## Initial scoping estimate

Tentative commit plan (refine in scope.md / commits.md):

1. **c01** — `rafaello/bundled.rs::resolve_runtime_binary` helper
   (release + workspace-target + env-override resolution; rejects
   shim-by-size or absent), with unit tests.
2. **c02** — `rfl init` post-materialise step: replace the shim
   at `target_dir.join(manifest.entry)` with the resolved runtime
   binary. In-tree acceptance test asserts the materialised
   entry is not the shim.
3. **c03** — `rfl init` regression test outside `cargo test`'s
   `CARGO_BIN_EXE_*` env leak (subprocess invocation of a
   `workspace_bin`-built `rfl` binary).
4. **c04** — `rfl-tui` handles Ctrl-C key event → Quit, with
   unit test.
5. **c05** — tmux-driven `rfl chat` Ctrl-C regression test.
6. **c06** (probable) — `manual-validation.md` v0.1.1 §1
   appendix: re-record the cold-start `rfl init && rfl chat`
   transcript end-to-end against the patched binary, plus a
   `^C → exit` capture. Mirrors m6 cK6.

That's 5–6 commits. Within the owner-set 5–10 budget; if scope.md
or commits.md rounds surface a missing test or a coupling, +1–2
slots feels tolerable.

**Possible drift commits at retrospective time**: `overview.md`
note on "bundled provider materialisation: runtime binary
resolved at install time, not at spawn time"; `decisions.md`
append row clarifying that shipped shim is *only* a
manifest-validator placeholder and never the spawn target;
`glossary.md` if a new term lands.

## Thorny investigation items to surface early

1. **"Does c03 stop at lock-correctness or extend to end-to-end
   spawn?"** Owner question. Tradeoff: end-to-end coverage costs
   a full syd/lockin spawn under test (slow, platform-sensitive,
   the macOS CI matrix is still ⏳ pending per m6 RATIFIED note);
   but the bug *manifests* at spawn time, not lock time, so
   stopping at lock-correctness leaves an inferential gap.
   My lean: c03 stops at "lock points at real binary"; the tmux
   Ctrl-C test (c05) implicitly exercises the full spawn path as
   a side effect, which gives us end-to-end coverage for free.
   But happy to be overruled.

2. **macOS Mach-O detection in c03.** If c03's assertion is
   "entry is not the shim", a byte-size heuristic (`> 1024`)
   works on both platforms. Magic-byte detection (ELF
   `\x7fELF` / Mach-O `\xfe\xed\xfa\xce` etc.) is more rigorous
   but adds platform code. Lean toward size heuristic.

3. **`bundled::resolve_runtime_binary` test-override semantics.**
   Tests need a way to point the resolver at a workspace-built
   `rfl-openai` without the env-vagueness of `CARGO_BIN_EXE_*`.
   Proposal: introduce `RFL_BUNDLED_BIN_OPENAI` env var (parity
   with `RFL_BUNDLED_PLUGINS_DIR`) — test sets it, prod ignores.
   Alternatively, extend `RFL_BUNDLED_PLUGINS_DIR` semantics so
   the resolver also looks for `<dir>/<name>/bin/<entry>` and
   accepts a non-shim file there. The latter avoids a second env
   var; the former is more explicit. scope.md round 1 will
   propose; pi gets to push back.

4. **Does the c01 helper belong in `rafaello/src/bundled.rs` or
   shared with the rafaello-core `compile::resolve_entry`
   layer?** I lean `bundled.rs` (it's a CLI-frontend concern —
   resolution by environment, not by lock content). But pi may
   argue for centralisation. Will be explicit in scope.md.

5. **Ctrl-C across Confirm overlay.** Today the overlay key
   handler dispatches into `handle_overlay_key`. The fix must
   match Ctrl-C *before* the mode-dispatch so it works in both
   Normal and ConfirmOverlay modes. Trivial in code but worth
   calling out in the scope acceptance line.

## Operational reminders to self

- Tmux session naming: `rafaello-v0_1_1-<role>` /
  `rafaello-v0_1_1-c<NN>-claude`. Never reuse.
- Worktrees under `/home/luiz/lab-wt/v0.1.1-*`. Pre-commit
  symlink at every worktree creation.
- `rm -rf` the agent worktree's `rafaello/target` after every
  ff-merge to main (disk hygiene — m4–m6 burned this three
  times).
- Don't self-CONVERGE retrospective.md / scope.md / commits.md
  — wait for explicit pi 0/0/0.
- Merge target is `main` directly (no `rafaello-v0.1` — that
  branch closed when v1 shipped).

## Open before kickoff

- Owner sanity-check on milestone-dir name (`m6.1-...` vs
  `m7-...`).
- Owner sign-off on c03 scope (lock-correctness vs end-to-end).
  → I'll propose the lean (lock-correctness) in scope.md round
  1; owner ratifies the answer at the scope.md ratification
  ping.

Will spawn scope.md drafting agent next.
