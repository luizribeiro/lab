# m6.1 scope.md — pi review round 2

Verdict: BLOCKING

Counts: B/2 M/1 N/3

## Blockers

### B-1: §A0's `resolve_plugin_dir` release fix is too broad and would regress `rfl install rfl-mailcat`

Round 2 correctly identifies the latent `rfl init` release-layout half of pi-1 B-1: `resolve_plugin_dir("openai")` cannot keep looking for `$out/share/rafaello/plugins/openai/` when `package.nix` writes `$out/share/rafaello/plugins/rfl-openai/`. The new §A0 lines 186-210 is therefore directionally necessary, and §A1's runtime-binary release path lines 222-228 now matches the ratified PP1 layout.

But the proposed §A0 implementation is specified as a blanket release-arm join from `name` to `rfl-<name>` (scope lines 198-201, internal split lines 640-652). `resolve_plugin_dir` is shared with `rfl install`: `install.rs:95-103` calls `bundled::resolve_plugin_dir(plugin)` with the operator's positional plugin string. For the shipped install UX that string is already `rfl-mailcat` (`rfl install rfl-mailcat`), while the release tree writes `$out/share/rafaello/plugins/rfl-mailcat/` (`package.nix:37-50`). A blanket `rfl-<name>` transform would look for `rfl-rfl-mailcat/` and break the release-installed `rfl install rfl-mailcat` path that m6 ratified.

The current resolver's release arm (`bundled.rs:27-36`) is wrong for init's internal name `openai`, but right for install's already-prefixed names. The fix needs a name-normalization rule, not a blind prefix.

Concrete fix proposal:

- Introduce a tiny helper for release plugin directory names, e.g. `release_plugin_dir_name(name) = if name.starts_with("rfl-") || name == "rafaello-fetch" { name } else { format!("rfl-{name}") }` (or a small explicit map for the bundled IDs if the `rafaello-fetch` exception should not be heuristic).
- Keep the dev-source crate mapping separate: `openai -> rafaello-openai`, `rfl-mailcat -> rafaello-mailcat`, `rfl-readfile -> rafaello-readfile`, etc. Leaving the dev fallback as `rafaello-{name}` is already known-bad for prefixed install names, even if m6.1 does not need to broaden dev install coverage.
- Add c01 unit cases for `resolve_plugin_dir("openai")` and `resolve_plugin_dir("rfl-mailcat")` in release layout. The second case is the regression guard that §A0 currently lacks.

Until this is corrected, the scope's new §A0 fix would repair release `rfl init` by regressing release `rfl install`, which is not acceptable for a patch milestone.

### B-2: §C4's tmux harness would not run the patched binary correctly or close the session after `rfl` exits

Round 2 correctly removes the invalid `waitpid`/`Pid::wait` plan from pi-1 M-3 (scope lines 478-487) and correctly requires a Ulid session nonce in the core C4 text (lines 457-461). However, the replacement tmux harness is still not a valid cK5-style regression anchor.

The key problem is lines 465-468: the test starts a generic tmux shell (`tmux new-session -d ...`) and then sends the text `rfl chat --project-root <tmp>` plus `C-m` into that shell. When `rfl chat` exits, the interactive shell remains alive, so `session_alive(&session)` will stay true and the test will fail even after the Ctrl-C fix. The text at lines 483-487 says “the shell-exec'd `rfl` finished and the pane closed,” but no `exec` is actually specified.

There are two more harness problems in the same block:

- The command uses bare `rfl`, not `workspace_bin("rfl")`. Cargo does not normally put `target/debug` on PATH for tmux's shell, and an installed/stale `rfl` on PATH would test the wrong binary.
- The scope polls `tmux capture-pane` for the parent stderr sentinel (lines 469-473). The cK5 precedent does the opposite: it writes a wrapper that exports `RFL_TUI_PATH`, redirects `rfl chat` stderr to a log, starts tmux with that wrapper as the pane command, and polls the stderr log for `frontend-ready-observed` (`rfl_chat_production_tui_input_overlay_e2e.rs:97-144`). That pattern exists because tmux does not pass arbitrary env reliably and the production TUI owns the alternate screen.

Concrete fix proposal:

- Build `workspace_bin("rfl")` and `workspace_bin("rfl-tui")` in C4.
- Create a cK5-style wrapper script and pass it as the command to `tmux new-session`, not as text typed into an interactive shell. The wrapper should at minimum set `TERM=xterm-256color`, export `RFL_TUI_PATH='<workspace rfl-tui>'`, and `exec '<workspace rfl>' chat --project-root '<tmp>' 2>'<stderr-log>'`.
- Poll the stderr log for `frontend-ready-observed`, as cK5 does, then send `C-c` to the session and poll `session_alive` for closure.
- If the scope wants “clean exit” stronger than “session closed + no panic,” have the wrapper write `$?` to a status file after the command returns. If it uses `exec`, status capture is not available; if status capture is required, run the command without `exec` in a non-interactive wrapper and `exit "$status"` after writing it.

As written, the demo-bar D2 test would be flaky or simply fail post-fix, so this remains blocking.

## Majors

### M-1: §C2 now depends on default target-dir layout but does not guard against `CARGO_TARGET_DIR`

The pi-1 M-1 fold is acceptable at the resolver-contract level: §A1 lines 233-240 explicitly limits dev fallback to the default `<workspace>/target/<profile>/` layout and names `RFL_BUNDLED_BIN_<NAME>` as the escape hatch for external target dirs.

But §C2 lines 398-414 still builds `rfl` and `rfl-openai` through `workspace_bin(...)`. That helper honors ambient `CARGO_TARGET_DIR` (`workspace_bin_path.rs:26-31`). If the test process runs with an external target dir, `rfl_path` will be outside the workspace; then the no-override subprocess intentionally required by §C2 cannot walk up from `<rfl-exe-parent>` to the workspace root. In other words, §A1 says this case is out of contract, while the demo-bar test does not skip or force the in-contract layout.

This is not a conceptual blocker for the owner-hit default checkout repro, but it is a real implementation-risk gap. Fix options:

- In C2, assert/skip early when `CARGO_TARGET_DIR` is set to an external path, with a message explaining that this regression specifically covers default-target `cargo run` layout; or
- build/copy the two binaries into `<workspace>/target/<profile>/` for this test regardless of ambient `CARGO_TARGET_DIR`; or
- add a separate explicit-override subprocess test for external target dirs and leave C2 as the default-layout test.

## Nits

### N-1: Owner-judgment item 5 still lists PID as an acceptable nonce

The main C4 text correctly mandates `Ulid::new()` (scope lines 457-461), closing pi-1 M-4. Owner-judgment item 5 still says “e.g. PID-suffix or `Ulid`-suffix” (lines 604-607). Drop the PID example there too so the owner-judgment list does not re-open the stale option.

### N-2: Owner-judgment item 6 still mentions “exit status 0 + no zombie”

Scope §C4 no longer attempts `waitpid`, zombie checks, or direct exit-status assertion (lines 478-487), which is the right fold for pi-1 M-3. Owner-judgment item 6 still says to limit C4 to “exit status 0 + no zombie + terminal restored” (lines 609-615). Update it to match the round-2 test contract: session exits within timeout, no panic in captured output/log, and no audit-log assertions.

### N-3: §C2 digest bullet says “not the digest of the shim” but describes equality with the current tree

Scope §C2 lines 426-429 says to assert the lock digest is “not the digest of the shim-as-entry,” then describes parsing the lock, recomputing `content_digest(target_dir)`, and asserting equality. The latter is the useful assertion and matches §A2's digest-ordering requirement; it does not by itself compute a shim digest. Reword to “Assert the lock digest equals `content_digest(target_dir)` after the swap,” and rely on the file-size / C1 exact-byte assertions for “not shim.”

## Items the scope handles correctly (confirmation)

- pi-1 B-1's runtime-binary half is correctly folded: §A1 lines 222-228 now points at `$out/share/rafaello/plugins/rfl-openai/bin/rfl-openai`, matching `package.nix:37-59` and decisions rows 59/65. The residual is only the shared `resolve_plugin_dir` name-normalization issue in B-1 above.
- pi-1 B-2 is correctly folded: §C2 lines 394-434 deliberately omits `RFL_BUNDLED_BIN_OPENAI` and leaves explicit override coverage to C1 lines 376-392.
- pi-1 M-1 is honestly folded in §A1 lines 233-240 by carving external `CARGO_TARGET_DIR` out of the dev-fallback contract. See M-1 above only for the test-environment guard.
- pi-1 M-2 is correctly folded: §B4 lines 347-369 now says no `confirm_reply` is synthesized and no audit-deny row is specified.
- pi-1 M-3's invalid `waitpid` primitive is removed from the main C4 body (lines 478-487). The remaining problem is the new tmux invocation shape in B-2 above.
- pi-1 M-5 is correctly folded: §C4 lines 494-498 now says C3 would fail pre-fix and C4 proves raw-mode delivery.
- pi-1 N-1/N-2/N-3/N-4 are all materially folded in the main text: §A1 lines 241-248, §A2 lines 259-263, §D lines 507-512, and acceptance summary lines 707-713.
- The digest-ordering, in-tree shim, and PP1 real-file containment rationales from round 1 remain correct (§A2 lines 264-269, §A4 lines 278-289, §A5 lines 291-299).

## Out-of-scope checks performed (negative coverage)

- Re-checked the PP1 release layout against `package.nix`; the runtime path is now correct, but source-dir name normalization must not regress existing prefixed install names.
- Re-checked that C2's removal of `RFL_BUNDLED_BIN_OPENAI` exercises the intended dev fallback. It does under default target layout; external target dirs need the M-1 guard.
- Re-checked the confirmation shutdown wording against `lib.rs` and `gate/mod.rs`; the round-2 no-implicit-deny text is now accurate.
- Re-checked the cK5 tmux precedent; the wrapper/log/new-session-as-command pattern is load-bearing for C4 and should be copied rather than replaced with an interactive shell command.
