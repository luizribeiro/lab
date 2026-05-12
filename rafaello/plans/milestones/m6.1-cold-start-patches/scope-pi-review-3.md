# m6.1 scope.md — pi review round 3

Verdict: NON-BLOCKING

Counts: B/0 M/0 N/2

## Blockers

None.

## Majors

None.

## Nits

### N-1: §A0 should explicitly state the env-override directory name used by `resolve_plugin_dir_for_bundled`

The round-3 `BundledPluginNames` split is the right fix for pi-2 B-1: it preserves `rfl install`'s existing `resolve_plugin_dir(plugin)` call site (`install.rs:95-103`) while giving `rfl init` a release-dir-aware path for `openai`. The field semantics are clear for release (`release_dir`) and dev workspace walk-up (`dev_crate`) at scope lines 292-321.

One small ambiguity remains: §A0 line 314 says `resolve_plugin_dir_for_bundled` “mirrors the existing `resolve_plugin_dir` layout resolution” but then only names the release and workspace arms. It should say what the `RFL_BUNDLED_PLUGINS_DIR` env arm joins. C1 still sets `RFL_BUNDLED_PLUGINS_DIR` to a temp directory mirroring `rafaello/crates/rafaello-openai/` under `openai/` (lines 502-504), so the intended env-arm lookup is presumably `<env-root>/<names.dev_crate>`.

Concrete wording fix: in §A0 step 2, add “env override: `<RFL_BUNDLED_PLUGINS_DIR>/<names.dev_crate>`” and add an env-arm unit test for `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`. This is non-blocking because C1 would catch the wrong implementation, but spelling it out prevents agent guesswork.

### N-2: §C2's `CARGO_TARGET_DIR` guard is conservative; align wording with the actual guard

The pi-2 M-1 fold is acceptable: §C2 now has an early skip when `CARGO_TARGET_DIR` is set (lines 536-545), matching §A1's contract that the no-override dev fallback only covers default target layout (lines 369-377). That prevents a false failure when `workspace_bin` builds into an external target dir.

The wording says the issue is “external `CARGO_TARGET_DIR`,” but the specified guard skips on any non-empty `CARGO_TARGET_DIR`, including a hypothetical in-workspace/default-equivalent target dir where walking up from `<workspace>/target/<profile>/rfl` would still work. This is a conservative skip, not a correctness bug. Suggested tweak: either say “when `CARGO_TARGET_DIR` is set” consistently, or make the guard actually test externality if the implementation wants to preserve coverage for in-workspace target dirs.

## Items the scope handles correctly (confirmation)

- **pi-2 B-1 folded correctly.** §A0 now introduces `BundledPluginNames` and the sister `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)` for init (lines 292-321), while explicitly leaving `resolve_plugin_dir(name)` unchanged for `rfl install` (lines 322-326). The c01 plan includes the regression guard `resolve_plugin_dir("rfl-mailcat")` release-arm hit (lines 749-763), which is exactly the missing protection from round 2.
- **pi-2 B-2 folded correctly.** §C4 now mirrors the cK5 wrapper-as-pane-command pattern: build workspace `rfl`/`rfl-tui`, write a wrapper exporting `RFL_TUI_PATH` and `TERM`, `exec` the workspace `rfl`, redirect stderr to a log, pass the wrapper to `tmux new-session`, poll the stderr log, send `C-c`, and poll `session_alive` (scope lines 621-698). This matches the cK5 precedent (`rfl_chat_production_tui_input_overlay_e2e.rs:97-144`, `:228-244`, `:267-275`). The `exec` semantics are correct: with one tmux pane running the wrapper, replacing the shell with `rfl` lets the session close when `rfl` exits; `session_alive` is implemented as `tmux has-session -t <name>` in cK5.
- **pi-2 M-1 folded adequately.** §C2's skip guard (lines 536-545) avoids asserting the no-override dev fallback under an ambient target dir that may be outside the workspace. C1 remains the explicit-override coverage (lines 497-513).
- **pi-2 N-1/N-2/N-3 folded.** Owner-judgment item 5 now requires `Ulid::new()` and removes PID (lines 727-733); item 6 now matches the C4 session-exit/no-panic/no-audit assertion surface (lines 734-741); §C2's digest bullet now says the lock digest equals post-swap `content_digest(target_dir)` (lines 564-570).
- **A1 env-var munging is reasonable.** Deriving `RFL_BUNDLED_BIN_<NAME_UPPER>` from `names.dev_crate` (lines 348-355) gives the stable logical override `RFL_BUNDLED_BIN_OPENAI`, not the noisier `RFL_BUNDLED_BIN_RFL_OPENAI`.
- **A2 source/runtime resolver pairing is clear.** §A2 lines 394-400 states that both `pp1::materialise`'s source tree and the runtime-binary swap are driven from the same `OPENAI_NAMES` record. This matches the intended `init.rs` edit: current `init.rs` calls `resolve_plugin_dir(BUNDLED_OPENAI)` before `pp1::materialise` (`init.rs:77`, `:104-109`), and m6.1 will switch that source-dir call to the new sister function.
- **Prior round-1 correctness confirmations still stand.** Digest recomputation after swap, keeping the in-tree shim for `validate_with_package`, real bytes rather than symlinks under PP1, child-side Ctrl-C handling, no audit-log assertion for Ctrl-C exit, and deferring Ctrl-D/Esc/parent signal refactors remain correctly scoped.

## Out-of-scope checks performed (negative coverage)

- Re-checked the PP1 release layout: `package.nix` writes plugin trees under `$out/share/rafaello/plugins/<plugin-bin-name>/bin/<plugin-bin-name>`, and the round-3 runtime path now follows that without moving plugin binaries back to top-level `$out/bin`.
- Re-checked `rfl install` preservation: because `resolve_plugin_dir(name)` remains unchanged, `rfl install rfl-mailcat` continues to pass the already-prefixed name through the existing resolver.
- Re-checked C4 against cK5's actual helpers, including wrapper `exec`, stderr-log polling, Drop guard, and `session_alive` via `tmux has-session`; no remaining blocking issue there.
- Re-checked the C2 no-runtime-override path: with default target layout and no `CARGO_TARGET_DIR`, walking up from `<workspace>/target/<profile>/rfl` finds the workspace root and then `<workspace>/target/<profile>/rfl-openai` as intended.
