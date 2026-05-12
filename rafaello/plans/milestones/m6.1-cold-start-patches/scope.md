# m6.1 — v0.1.1 cold-start patches — scope

> **Status:** round 4 — claude-authored 2026-05-12, awaiting
> pi round 4. Folds `scope-pi-review-3.md` (0B / 0M / 2N,
> NON-BLOCKING) on top of round 3. Target verdict: CONVERGED.
>
> **Round-4 changelog (both pi-3 nits folded):**
>
> - **N-1.** §A0 step 2 now explicitly names the
>   `RFL_BUNDLED_PLUGINS_DIR` env-arm join used by
>   `resolve_plugin_dir_for_bundled`:
>   `<RFL_BUNDLED_PLUGINS_DIR>/<names.dev_crate>` (e.g.
>   `<env-root>/openai/`). c01 unit tests add the env-arm
>   case for `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`.
> - **N-2.** §C2's `CARGO_TARGET_DIR` guard wording aligned
>   with the actual guard predicate. The guard is
>   conservative (skip on any non-empty `CARGO_TARGET_DIR`,
>   not only external paths) — round-4 wording now says
>   "when `CARGO_TARGET_DIR` is set" consistently and the
>   round-3 "external" phrasing is removed. Tightening the
>   guard to test externality is deferred (filed as a
>   follow-up): not worth the path-canonicalisation
>   complexity for a regression test.
>
> Cumulative trajectory: round 1 → 2B/5M/4N (BLOCKING) →
> round 2 → 2B/1M/3N (BLOCKING) → round 3 → 0B/0M/2N
> (NON-BLOCKING) → round 4 (this commit), target verdict
> CONVERGED.
>
> ---
>
> **(History — round 3 status, preserved for traceability.)**
>
> Round 3 — claude-authored 2026-05-12. Folds
> `scope-pi-review-2.md` (2B / 1M / 3N, BLOCKING) on top of
> round 2.
>
> **Round-3 changelog (every pi-2 finding folded):**
>
> - **B-1 (§A0 blanket prefix would regress `rfl install
>   rfl-mailcat`).** Round 2 had `resolve_plugin_dir`'s
>   release-arm blindly prefix `rfl-` to every input name.
>   But `rfl install` already passes the prefixed name
>   (`install.rs:96` calls `bundled::resolve_plugin_dir(plugin)`
>   with the operator's positional string `rfl-mailcat`),
>   which today's release-arm resolves correctly. Adding a
>   blanket prefix would make it look for
>   `share/rafaello/plugins/rfl-rfl-mailcat/` and regress the
>   m6 install path. **Round-3 fix**: drop the
>   `resolve_plugin_dir`-signature change entirely. Instead,
>   §A0 introduces a tiny `pub struct BundledPluginNames {
>   dev_crate: &'static str, release_dir: &'static str,
>   runtime_bin: &'static str }` plus a new sister function
>   `bundled::resolve_plugin_dir_for_bundled(&BundledPluginNames)`
>   that uses `release_dir` (e.g. `"rfl-openai"`) on the
>   release arm and `dev_crate` (e.g. `"openai"`) for the
>   workspace-walk-up arm. `init.rs` switches to the new
>   function with a single `OPENAI_NAMES` constant. **The
>   existing `resolve_plugin_dir(name)` retains its current
>   behaviour** so `rfl install` is untouched. The
>   sister-function approach surfaces the dev-vs-release
>   name asymmetry explicitly (a v1-design wart that m6
>   shipped without exercising both arms) without
>   broadening m6.1 into a `rfl install` refactor.
>   Resolves pi-2 B-1.
> - **B-2 (round-2 §C4 tmux harness would leave the shell
>   alive after `rfl` exits).** Round 2 sent the command as
>   text into an interactive shell. When `rfl chat` exits,
>   the shell stays alive and `session_alive` returns true
>   forever. **Round-3 fix**: §C4 rewritten to match cK5's
>   wrapper-script-as-pane-command pattern verbatim
>   (`rfl_chat_production_tui_input_overlay_e2e.rs:97-144`).
>   The test writes a `rfl-chat-wrapper.sh` that exports
>   `RFL_TUI_PATH=<workspace rfl-tui>`, `TERM=xterm-256color`,
>   and `exec`s `workspace_bin("rfl")` with stderr redirected
>   to a log file; passes the wrapper as the
>   `tmux new-session` command (not as keyed text); polls
>   the stderr log for `frontend-ready-observed`; sends
>   `C-c`; polls `session_alive` for closure. Uses
>   `workspace_bin("rfl")` / `workspace_bin("rfl-tui")` so
>   no `$PATH` `rfl` can shadow the test target. Resolves
>   pi-2 B-2.
> - **M-1 (§C2 honors external `CARGO_TARGET_DIR`).**
>   `workspace_bin` honors ambient `CARGO_TARGET_DIR`
>   (`workspace_bin_path.rs:26-31`), so under an external
>   target dir, the `<rfl-exe-parent>` walk-up cannot reach
>   the workspace root. **Round-3 fix**: §C2 starts with an
>   early `if std::env::var_os("CARGO_TARGET_DIR")
>   .map(|v| !v.is_empty()).unwrap_or(false) { eprintln!("…
>   covers default target-dir layout only — set
>   RFL_BUNDLED_BIN_OPENAI as an override or unset
>   CARGO_TARGET_DIR for this regression"); return; }` skip
>   guard. The explicit-override case stays covered by C1.
>   Resolves pi-2 M-1.
> - **N-1.** Owner-judgment item 5 updated: `Ulid::new()` is
>   the requirement; the PID-suffix example removed.
> - **N-2.** Owner-judgment item 6 updated to match the
>   round-2 C4 contract: session exits within timeout, no
>   panic in captured pane/log, no audit-log assertions.
>   "Exit status 0 + no zombie + terminal restored"
>   language removed.
> - **N-3 (§C2 digest bullet self-contradiction).** Round-2
>   said "not the digest of the shim-as-entry, then
>   recompute equal." Round 3 keeps only the meaningful
>   assertion: "the lock digest equals
>   `content_digest(target_dir)` after the swap." The "not
>   shim" defence sits in C1's exact-bytes check + C2's
>   size > 1024 check.
>
> Cumulative trajectory: round 1 → 2B/5M/4N (BLOCKING) →
> round 2 → 2B/1M/3N (BLOCKING) → round 3 (this commit),
> target verdict CONVERGED.
>
> ---
>
> **(History — round 2 status, preserved for traceability.)**
>
> Round 2 — claude-authored 2026-05-12. Folds
> `scope-pi-review-1.md` (2B / 5M / 4N, BLOCKING) on top
> of round 1.
>
> **Round-2 changelog (every pi-1 item folded):**
>
> - **B-1 (PP1 release-layout mismatch).** Round-1 §A1 had
>   the release-arm at `<rfl-exe-parent>/rfl-<name>` (top-level
>   `bin/`). m6's RATIFIED PP1 contract (`decisions.md`
>   rows 59/65; `rafaello/nix/package.nix:37-63`) removes
>   `$out/bin/<plugin-bin>` after copying into
>   `$out/share/rafaello/plugins/<plugin-bin-name>/bin/<plugin-bin-name>`,
>   where `<plugin-bin-name>` is `rfl-<name>` (e.g.
>   `rfl-openai`). Round-2 §A1 release-arm is now
>   `<rfl-exe-parent>/../share/rafaello/plugins/rfl-<name>/bin/rfl-<name>`,
>   matching live nix package layout. **Plus**: round-2 §A1
>   surfaces a latent m6 bug in `bundled::resolve_plugin_dir`
>   itself (`bundled.rs:27-36`) — it looks for
>   `<bin>/../share/rafaello/plugins/<name>/` but `package.nix`
>   writes `rfl-<name>/`. Round 2 adds **§A0** (new) which
>   fixes `resolve_plugin_dir`'s release-arm to use the
>   `rfl-<name>/` naming. Without §A0, a released `rfl init`
>   could not find the bundled-source tree even before the
>   runtime-binary swap. (No nix-released `rfl init` flow
>   exists in the wild yet — m6 ratified the layout but
>   didn't exercise `rfl init` against it — so this is a
>   latent m6 cold-start bug that m6.1 picks up.)
> - **B-2 (C2 used the override, not the dev fallback).**
>   Round-1 §C2 allowlisted `RFL_BUNDLED_BIN_OPENAI`, which
>   short-circuits the resolver and proves only the explicit
>   override works. Round 2 §C2 drops `RFL_BUNDLED_BIN_OPENAI`
>   entirely; the subprocess `rfl init` invocation runs with
>   `RFL_BUNDLED_PLUGINS_DIR` pointed at the in-tree fixture
>   *and lets the runtime-binary dev-fallback walk up from
>   `<rfl-exe-parent>` to find `target/<profile>/rfl-openai`*.
>   That is the truest reproduction of the owner-hit
>   `cargo run --bin rfl -- init` cold-start path. The explicit
>   `RFL_BUNDLED_BIN_OPENAI` override remains covered by §C1's
>   in-process test (where the exact-byte assertion lives).
> - **M-1 (CARGO_TARGET_DIR external dev fallback).**
>   Round-2 §A1 explicitly carves out the external
>   `CARGO_TARGET_DIR` case: the dev fallback covers the
>   default `cargo run --bin rfl` layout
>   (`<workspace>/target/<profile>/rfl`) only;
>   `RFL_BUNDLED_BIN_<NAME>` is the documented escape hatch
>   for non-default target dirs. No second fallback added —
>   keeps the resolver simple, and the escape hatch is
>   one env var.
> - **M-2 (invented implicit-deny path).** Round-2 §B4 drops
>   the "parent treats unanswered confirmations as session
>   torn down" claim — that path does not exist in the gate
>   (`rafaello-core/src/gate/mod.rs` only resolves on
>   `core.session.confirm_reply` or timeout). Round-2 §B4
>   reworded to: Ctrl-C exits the TUI; the parent observes
>   frontend exit via `handle.wait()` and runs the existing
>   shutdown sequence; no `confirm_reply` is synthesised; no
>   audit-deny row is specified for m6.1.
> - **M-3 (Pid::wait is wrong primitive for tmux child).**
>   Round-2 §C4 follows the m6 cK5 precedent
>   (`rfl_chat_production_tui_input_overlay_e2e.rs:228-244`):
>   poll `session_alive(&session)` with a bounded timeout
>   after `tmux send-keys C-c`; assert the session exits
>   within the timeout; use the cK5-style Drop guard to
>   `tmux kill-session` on test teardown so failures do not
>   leak sessions. No `waitpid` on a non-child process.
> - **M-4 (Ulid nonce, not PID).** Round-2 §C4 mandates
>   `Ulid::new()` in the session name (cK5 used PID, which
>   suffices when there is a single `#[test]` in the binary
>   but is fragile across stale-session cleanup; m6.1 takes
>   the more defensive nonce).
> - **M-5 (false pre-fix unit-test claim).** Round-2 §C4
>   reworded: "The C3 unit test would fail pre-fix and pass
>   post-fix; C4 is required to prove the real raw-mode TTY
>   delivery path." Removed the wrong "unit test would have
>   passed before the fix" line.
> - **N-1.** Round-2 §A1 not-found message now names
>   `RFL_BUNDLED_BIN_<NAME_UPPER>` literally (e.g.
>   `RFL_BUNDLED_BIN_OPENAI`), not `<NAME>_BIN_OPENAI`.
> - **N-2.** Round-2 §A2 step 4 reworded — "preserve /
>   normalize executable mode after the swap" instead of
>   "resolver may return non-exec".
> - **N-3.** Round-2 §D `01-cold-init.txt` clarified: runs
>   with `--yes` (no interactive prompt), matching C2 and
>   the m6 README bootstrap. Drop the "accept prompt shown"
>   wording.
> - **N-4.** Round-2 acceptance summary reworded:
>   "No code outside the named D1/D2 surfaces touched" (not
>   the over-absolute "no other code touched").
>
> Cumulative trajectory: round 1 → 2B/5M/4N (BLOCKING) →
> round 2 (this commit), target verdict CONVERGED or
> NON-BLOCKING.
>
> ---
>
> **(History — round 1 status, preserved for traceability.)**
>
> Round 1 — claude-authored 2026-05-12. First post-v1 patch
> milestone. Forked off `main` at `1e0ba53` (the m6 RATIFIED
> commit declaring v1 demo-ready). Driver-preflight `821e5c0`.

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

**A0. (Round 3 — closes pi-2 B-1.)** A `rfl init` against the
nix-released layout needs the bundled-source tree resolved
under `$out/share/rafaello/plugins/rfl-openai/`, not
`$out/share/rafaello/plugins/openai/`. Today
`bundled::resolve_plugin_dir("openai")` looks for the latter
(`bundled.rs:27-36`), so a released `rfl init` would fail with
"bundled plugin not found" before even reaching the
runtime-binary swap. This is a latent m6 bug: m6 RATIFIED the
release layout in `package.nix` (which writes `rfl-<name>/`)
but no test exercised `rfl init` against that layout.

**Round-3 fix shape (does NOT touch `rfl install`):**

1. Introduce
   ```rust
   pub struct BundledPluginNames {
       /// Crate name suffix for the dev-fallback workspace
       /// walk-up. `crates/rafaello-<dev_crate>/`.
       pub dev_crate: &'static str,
       /// Release-tree plugin directory under
       /// `$out/share/rafaello/plugins/<release_dir>/`.
       pub release_dir: &'static str,
       /// Release-tree runtime binary name under
       /// `<release_dir>/bin/<runtime_bin>` and dev-target
       /// `<workspace>/target/<profile>/<runtime_bin>`.
       pub runtime_bin: &'static str,
   }

   pub const OPENAI_NAMES: BundledPluginNames =
       BundledPluginNames {
           dev_crate: "openai",
           release_dir: "rfl-openai",
           runtime_bin: "rfl-openai",
       };
   ```
   in `rafaello/src/bundled.rs`. The struct is `pub` so the
   §A1 resolver can take it, and so future tests can
   declare similar fixtures.
2. New function
   `bundled::resolve_plugin_dir_for_bundled(names:
   &BundledPluginNames) -> Result<PathBuf, BundledError>`
   that mirrors the existing `resolve_plugin_dir` three-arm
   resolution:
   - **env arm**:
     `<RFL_BUNDLED_PLUGINS_DIR>/<names.dev_crate>` (e.g.
     `<env-root>/openai/`). The dev-crate axis is the
     stable logical identifier — same axis the §A1
     `RFL_BUNDLED_BIN_<NAME_UPPER>` munge derives from —
     and lets C1's in-process test mirror the in-tree
     `rafaello/crates/rafaello-openai/` fixture under a
     temp `<env-root>/openai/` directory.
   - **release arm**: joins `names.release_dir`
     (e.g. `rfl-openai`) under
     `<rfl-exe-parent>/../share/rafaello/plugins/`.
   - **dev fallback**: walks up from `<rfl-exe-parent>`
     looking for a workspace `Cargo.toml`; if found,
     returns `<workspace>/crates/rafaello-<names.dev_crate>/`.
3. `init.rs::run` switches from
   `bundled::resolve_plugin_dir(BUNDLED_OPENAI)` to
   `bundled::resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`,
   and `BUNDLED_OPENAI` is removed (replaced by
   `OPENAI_NAMES`).
4. **`bundled::resolve_plugin_dir(name)` retains its
   current signature and behaviour**, so the `rfl install`
   call site at `install.rs:96` continues to resolve
   `rfl-mailcat` → `share/rafaello/plugins/rfl-mailcat/`
   exactly as it does today. m6.1 explicitly does **not**
   refactor `rfl install`'s name handling — that
   dev/release naming asymmetry is a v0.1.x follow-up
   candidate (`§Follow-ups`).

Why a sister function rather than parameterising the
existing one: the existing `resolve_plugin_dir(name)` takes
a single `name` because its callers (live: `rfl install`)
have only one name to give. Parameterising it to also take
release-vs-dev names would force every call site (and the
helper's caller in `install.rs`) to know the asymmetry. The
sister function isolates the new shape to the bundled-init
path that needs it.

**c01 unit tests** cover both functions explicitly:
- `resolve_plugin_dir("rfl-mailcat")` release-arm hit
  (regression guard — proves m6.1 did not break install).
- `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`
  env-arm hit (via `RFL_BUNDLED_PLUGINS_DIR`) +
  release-arm hit + dev-fallback hit.
- `resolve_runtime_binary(&OPENAI_NAMES)` env-override hit,
  release-arm hit, dev-fallback hit, not-found.

**A1.** New helper
`rafaello::bundled::resolve_runtime_binary(names:
&BundledPluginNames) -> Result<PathBuf, BundledError>`.
Resolution order:

1. `RFL_BUNDLED_BIN_<NAME_UPPER>` env var (test override;
   explicit per CLAUDE.md "errs-toward-explicit-config").
   `<NAME_UPPER>` is derived from `names.dev_crate` (hyphens
   → underscores, uppercased — e.g. `openai` →
   `RFL_BUNDLED_BIN_OPENAI`). The dev-crate axis is used
   because it is the stable logical identifier; the
   `runtime_bin` axis carries the `rfl-` prefix which
   would yield `RFL_BUNDLED_BIN_RFL_OPENAI` and is
   noisier. Value is the absolute path to the binary;
   resolver validates it is a regular file and executable,
   else `BundledError::NotFound`.
2. **Release layout.**
   `<rfl-exe-parent>/../share/rafaello/plugins/<names.release_dir>/bin/<names.runtime_bin>`.
   For `OPENAI_NAMES` this resolves to
   `<rfl-exe-parent>/../share/rafaello/plugins/rfl-openai/bin/rfl-openai`,
   matching the m6 RATIFIED PP1 contract (`decisions.md`
   rows 59/65; `rafaello/nix/package.nix:37-63`): top-level
   `$out/bin/` retains only `rfl` and `rfl-tui`, while every
   plugin runtime binary lives at
   `$out/share/rafaello/plugins/<release_dir>/bin/<runtime_bin>`.
3. **Dev fallback.** Walk up from `<rfl-exe-parent>` looking
   for a workspace root (`Cargo.toml` containing `[workspace]`);
   if found, return
   `<workspace>/target/<profile>/<names.runtime_bin>` where
   `<profile>` is `debug` in `cfg!(debug_assertions)` else
   `release`. Covers the default `cargo run --bin rfl`
   layout (`<workspace>/target/<profile>/rfl`). **External
   `CARGO_TARGET_DIR` is explicitly out of the dev-fallback
   contract — `RFL_BUNDLED_BIN_<NAME>` is the documented
   escape hatch** for non-default target dirs (CI containers,
   `cargo --target-dir`, `nix develop` with a relocated
   target). No second fallback added; the resolver stays
   simple and the escape hatch is a single env var.
4. None of the above → `BundledError::NotFound { name:
   names.runtime_bin.to_owned() }` with the message `"no
   <runtime_bin> runtime binary discoverable (tried env
   RFL_BUNDLED_BIN_<NAME_UPPER>,
   <rfl-exe-parent>/../share/rafaello/plugins/<release_dir>/bin/<runtime_bin>,
   workspace target/<profile>/<runtime_bin>)"`. The literal
   env name in the message resolves at runtime via the
   `<NAME_UPPER>` munge so `openai` shows
   `RFL_BUNDLED_BIN_OPENAI` (pi-1 N-1 fix).

**A2.** In `init::run`, after `pp1::materialise(...)` returns
`target_dir`:

1. Resolve the runtime binary via
   `bundled::resolve_runtime_binary(&OPENAI_NAMES)`. Note
   that `pp1::materialise` is itself called with the source
   tree from
   `bundled::resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`
   (§A0), so both the source-tree and runtime-binary
   resolutions consult the same `BundledPluginNames`
   record.
2. Compute `entry_absolute = target_dir.join(manifest.entry.as_str())`.
   This file already exists (the shim copied by PP1).
3. `fs::copy(runtime, &entry_absolute)` — overwrite the shim
   with the real binary bytes.
4. `fs::set_permissions(&entry_absolute, 0o755)` — preserve
   and normalise executable mode after the swap (`fs::copy`
   preserves the source's mode bits on Unix, but the
   defensive chmod guarantees the materialised entry is
   exec-able regardless of the resolved binary's source mode).
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
open exits the TUI. The parent observes frontend exit via
`handle.wait()` (`rafaello/src/lib.rs:751`) and runs the
existing shutdown sequence
(`run_post_ready` returns → step C11–C12 at lines 578–585
signals shutdown, joins router/agent/slash tasks, aborts the
gate task, drops plugin handles, calls
`plugin_supervisor.shutdown()`).

**No `confirm_reply` is synthesised** on Ctrl-C exit. The
gate (`rafaello-core/src/gate/mod.rs`) only resolves a pending
confirmation on `core.session.confirm_reply` or timeout; an
in-flight confirmation at Ctrl-C time is simply abandoned
when its observer tasks are torn down by the broker shutdown.
No audit-deny row is specified for m6.1 — m6's RATIFIED
contract does not specify Ctrl-C-during-confirm audit
semantics, and m6.1 is not the milestone to invent them.
Acceptance test asserts only that the Ctrl-C key-event
handler returns `Quit` in both modes (table-driven unit
test); C4 asserts only that the tmux session exits cleanly.

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

**C2. (D1 cold-start regression, subprocess — no runtime
override, exercises the dev fallback.)** New test
`rafaello/crates/rafaello/tests/rfl_init_runtime_binary_outside_cargo_env.rs`:

- **Skip guard.** If
  `std::env::var_os("CARGO_TARGET_DIR")
  .map(|v| !v.is_empty()).unwrap_or(false)`, print a
  diagnostic ("rfl_init_runtime_binary_outside_cargo_env:
  covers default target-dir layout only — set
  RFL_BUNDLED_BIN_OPENAI or unset CARGO_TARGET_DIR") and
  return. The guard is **conservative**: it skips whenever
  `CARGO_TARGET_DIR` is set, even if the value happens to
  point at the workspace-default `<workspace>/target`. The
  alternative (canonicalise the env value and compare
  against the workspace root) buys a small coverage edge
  case at the cost of path-canonicalisation complexity in
  a regression test, which is not worth it. The C1
  in-process test covers the explicit-override path on
  external-target hosts; the no-override dev-fallback is
  the only path C2 cares about.
- Build `rfl` via `workspace_bin("rfl")` and the bundled
  runtime via `workspace_bin("rfl-openai")` (the real
  workspace-target binary). Both land at
  `<workspace>/target/<profile>/`.
- Spawn `rfl` as `std::process::Command::new(rfl_path).arg("init")
  .arg("--yes").arg("--project-root").arg(tmp)`,
  **`.env_clear()`** followed by an explicit allowlist:
  `PATH` (so the resolver-internal `current_exe` resolution +
  any shell-out works), `HOME` (some user-config probes
  short-circuit cleanly with it set), and
  `RFL_BUNDLED_PLUGINS_DIR` pointed at a temp copy of the
  in-tree `rafaello/crates/rafaello-openai/` fixture (manifest
  + openrpc + the in-tree shim). **`RFL_BUNDLED_BIN_OPENAI` is
  deliberately NOT set** — the test exercises the §A1
  dev-fallback arm which walks up from `<rfl-exe-parent>` (=
  `<workspace>/target/<profile>/`) to find
  `<workspace>/target/<profile>/rfl-openai`. The `.env_clear()`
  is the critical step that reproduces the bug — without it,
  `CARGO_BIN_EXE_*` from the parent `cargo test` invocation
  leaks through and the bug self-heals.
- Assert exit status 0.
- Assert the materialised entry file is the real binary
  (size > 1024 bytes — heuristic, platform-neutral; magic-byte
  ELF/Mach-O detection is more rigorous but adds platform
  code and the size heuristic is sufficient when paired with
  C1's exact-bytes assertion. Pi-1 confirmed
  `target/debug/rfl-openai-stub` is ~29 MB; release builds are
  ~hundreds of KB; both >> 1024).
- Assert the lock's `[plugins.<canonical>] digest` field
  equals `content_digest(target_dir)` recomputed against the
  post-swap install dir. This is the meaningful assertion:
  it proves §A2 step 5 ran the digest *after* the binary
  swap (a stale-shim digest would not equal the recomputed
  one). The "not the shim" defence sits in C1's exact-bytes
  assertion + this test's size > 1024 check.
- **Stop here** — does not invoke `rfl chat`. Lock-correctness
  is what C2 proves; the full spawn path is exercised
  indirectly by C4. (Owner-ratified 2026-05-12.) The
  *explicit* `RFL_BUNDLED_BIN_<NAME>` override path is
  covered by C1's in-process exact-bytes test, not by C2.

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

**C4. (D2 end-to-end, tmux-driven — cK5 wrapper-script
pattern verbatim.)** New test
`rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`.
Mirrors `rfl_chat_production_tui_input_overlay_e2e.rs` lines
47–275 closely:

- **Tmux availability.** Skip with a printed message if
  `tmux -V` fails (cK5 lines 47–48). No setup happens
  otherwise.
- **Project setup.** Reuse the m6
  `install_demo_layout(InstallOptions { provider_executable:
  true, tool_executable: true, real_binaries: true })`
  helper (`tests/common/m4_install.rs:36-83`) under a
  `tempfile::tempdir()` project root.
- **Workspace binaries.** Build
  `rfl = workspace_bin("rfl")` and
  `tui = workspace_bin("rfl-tui")`. These resolve via the
  existing helper so the test always exercises the
  workspace-target binary, not any `$PATH` `rfl` that
  could shadow it.
- **Wrapper script.** Write a `rfl-chat-wrapper.sh` into
  the temp dir with content (verbatim adapt of cK5
  lines 102–121):
  ```sh
  #!/bin/sh
  export RFL_TUI_PATH='<tui-path>'
  export TERM='xterm-256color'
  exec '<rfl-path>' chat --project-root '<project-root>' 2>'<stderr-log>'
  ```
  chmod `0o755`. The `exec` is load-bearing: when `rfl
  chat` exits, the wrapper's PID is `rfl`'s PID (no
  intermediate shell), so the tmux pane closes with `rfl`'s
  exit and `session_alive` flips to false. **Without
  `exec`**, the wrapper shell stays alive after `rfl chat`
  exits, and the session-liveness regression check would
  fail forever.
- **Tmux session name.** `format!("rfl-c05-ctrlc-{}",
  Ulid::new())`. Ulid nonce (not PID) to survive
  stale-session cleanup and concurrent test runs.
- **Drop guard.** A `TmuxSessionGuard` struct holding the
  session name, with a `Drop` impl that runs
  `tmux kill-session -t <name>` (cK5 lines 267–275). The
  test creates the guard immediately after `tmux new-session`
  so any panic between session creation and clean teardown
  still cleans up.
- **Session start.**
  ```text
  tmux new-session -d -s <name> -x 100 -y 30 <wrapper-script>
  ```
  Passing the wrapper as the pane command — **not** as
  text typed into an interactive shell — so the pane's
  child is the wrapper (and after `exec`, `rfl` itself),
  not a long-lived `bash`.
- **Wait for ready.** Poll the `<stderr-log>` file for the
  literal `frontend-ready-observed` substring with a 30s
  timeout. cK5 calls this pattern `poll_for_stderr_line`
  (lines 142–148). The parent rafaello binary writes this
  sentinel to its own stderr at
  `lib.rs:661` (`write_parent_sentinel(..,
  "rfl-chat: frontend-ready-observed")`); the wrapper's
  `2><stderr-log>` redirect captures it.
- **Send Ctrl-C.** `tmux send-keys -t <name> C-c` — sends
  the raw Ctrl-C byte. **NB**: this is the byte the kernel
  TTY layer would convert to SIGINT *if* raw mode were
  disabled; in raw mode, the byte arrives as a `KeyEvent`
  with `KeyModifiers::CONTROL + KeyCode::Char('c')`. The
  §B1 fix converts it to `EventOutcome::Quit`.
- **Wait for exit.** Poll `session_alive(&name)` with a
  5s bounded timeout (cK5's post-`q` wait is similarly
  small). `session_alive` shells out to `tmux has-session
  -t <name>` and returns true iff exit status is 0 (cK5
  pattern).
- **Assert.** The session exited within the timeout (i.e.
  `session_alive` returned false). If it did not, capture
  the pane and the stderr log and emit a diagnostic with
  both included so failures are debuggable (cK5
  lines 240–245 precedent).
- **Defensive checks.** Read the captured stderr log and
  assert no `thread '<name>' panicked` appears (catches
  panics in the parent rafaello process during shutdown).
  Capture-pane the post-exit screenful (best-effort —
  the pane may already be gone) and check the same.
- **No audit-log assertions.** §B4 explicitly carves
  audit-row semantics for Ctrl-C-during-confirm out of
  m6.1. The test ignores audit-log contents and the
  session-SQLite contents.

This is the load-bearing regression anchor for D2. The C3
unit test would fail pre-fix and pass post-fix (proves the
handler converts the key event to `Quit`), but only C4
proves the real raw-mode TTY path delivers Ctrl-C to the
handler and the parent/child stack tears down cleanly.

### §D — `manual-validation.md` v0.1.1 §1 appendix

**D1.** Append a `manual-validation.md` v0.1.1 §1 subsection
(under `rafaello/plans/milestones/m6.1-cold-start-patches/manual-validation.md`,
new file — mirrors m6 cK6's `section-5-phase-k/` appendix
pattern). Three captures:

1. `01-cold-init.txt` — `rfl init --yes` against an empty
   project root from a freshly-built `rfl` (no
   `CARGO_BIN_EXE_*` env ambient). Asserted by visual
   inspection: lock written, no error, no interactive
   accept prompt rendered (the `--yes` flag short-circuits
   the prompt — matches C2 and the m6 README bootstrap).
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
5. **C4 tmux session name nonce.** **Resolved round 3**:
   `Ulid::new()` (per pi-2 M-4 / N-1). PID is not
   sufficient under stale-session cleanup or parallel test
   runs; cK5's PID-suffix worked because cK5 is the only
   `#[test]` in its binary, but m6.1 takes the defensive
   nonce.
6. **C4 assertion surface.** **Resolved round 3 (per
   pi-2 N-2)**: limit C4 to "tmux session exits within
   bounded timeout, no Rust panic in captured pane/stderr
   log, no audit-log or session-SQLite assertions". The
   round-1 "exit status 0 + no zombie + terminal restored"
   language is dropped because §C4 no longer attempts a
   `waitpid` reap (pi-1 M-3, resolved round 2).
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

1. **c01** — `feat(rafaello): BundledPluginNames +
   resolve_plugin_dir_for_bundled + resolve_runtime_binary`.
   Three additions in `rafaello/src/bundled.rs`:
   (a) `BundledPluginNames` struct + `OPENAI_NAMES` const
   per §A0;
   (b) `resolve_plugin_dir_for_bundled(&BundledPluginNames)`
   sister function that mirrors `resolve_plugin_dir` but
   uses `release_dir` for the release-arm join and
   `dev_crate` for the workspace walk-up (§A0);
   (c) `resolve_runtime_binary(&BundledPluginNames)` per §A1.
   The existing `resolve_plugin_dir(name)` is **not** modified;
   `rfl install`'s call site remains untouched. Unit tests in
   the same file cover: `resolve_plugin_dir("rfl-mailcat")`
   release-arm regression guard; `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`
   release + dev arms; `resolve_runtime_binary(&OPENAI_NAMES)`
   env-override / release / dev-fallback / not-found. Under
   the 100-line / 3–5-file commit limit.
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
- **v0.1.x candidate:** rationalise the dev-crate vs
  release-dir vs runtime-bin naming asymmetry across the
  bundled-plugin set. Today: `rafaello-fetch` ships *without*
  the `rfl-` prefix in both its bin name and release dir
  (`package.nix:25` line `"rafaello-fetch:rafaello-fetch"`),
  while every other bundled plugin uses `rfl-<x>`. m6.1 does
  not touch this — `resolve_plugin_dir(name)` stays as-is,
  and `BundledPluginNames` records the asymmetry per plugin
  if any future `rfl init`-style bundled-init grows for
  another plugin. A v0.2 cleanup could unify the naming
  schema and shrink the `BundledPluginNames` struct to a
  single name field.
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
- No code outside the named D1/D2 surfaces touched
  (`rafaello/src/{bundled.rs,init.rs}`,
  `rafaello-tui/src/bin/rfl_tui.rs`, the four named test
  files, the appendix transcripts, and one `decisions.md`
  row appended at retrospective time). No new commands. No
  `overview.md` structural drift. No regression sweep
  beyond the two defects.

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

None across rounds 1–3. All four blockers (pi-1 B-1/B-2,
pi-2 B-1/B-2), all six majors, and all nine nits across the
three review rounds were substantive and accurate; every
one is folded. No standing disagreement.
