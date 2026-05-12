# m6 retrospective.md round-3 pi review

> Verdict: non-blocking
> Counts: B/0 M/0 N/3

Round 3 closes the implementation blockers from round 2. The `env_clear()` syd-pty regression is fixed at the lockin layer, the regression test exercises the exact strip-and-spawn path and passes on the current tree, and the refreshed §5 audit/sqlite/wire artifacts are honest live evidence. The remaining rendered-TUI gap is correctly framed as an owner-routing question rather than silently claimed as closed.

## Stdio owner-routing verdict

The `Stdio::null()` finding is real: `rafaello/crates/rafaello-core/src/frontend/mod.rs:203-205` pipes stderr but nulls stdin/stdout for the `rfl-tui` child, while production `rfl-tui` renders to `io::stdout()` and reads terminal events. That explains the blank tmux captures; the render is going to `/dev/null`, not to the tmux pty.

Option B is mechanically small: drop the `stdin(Stdio::null())` / `stdout(Stdio::null())` calls (or replace them with inherited stdio) so the TUI subprocess shares the parent pty. I do **not** think it is so trivially architecture-correct that pi should force it without owner routing: it changes the frontend supervisor's production stdio contract, and the old null-stdio shape may have been chosen for non-interactive/scripted runs. Tests using `RFL_TUI_TEST_MODE=1` are likely unaffected, but production terminal ownership is a scope/API decision. The round-3 framing is therefore defensible: owner may ratify on wire-shape + audit evidence, or require the stdio amendment and a recapture pre-merge.

## Round-2 follow-up table

| Prior finding | Round-3 status |
|---|---|
| B1 `env_clear()` regression breaks cold start / right-layer syd-pty | **Closed.** `SandboxedCommand::apply_sandbox_internal_env` re-applies `CARGO_BIN_EXE_syd-pty` before `spawn` / `status` / `output` in sync and tokio paths. The supervisor still calls `cmd.env_clear()`, so lockin owns the preservation. The new fake-syd test passes now and would fail against the pre-fix path because `env_clear()` removed the syd-pty env before fake-syd recorded environ. |
| B2 blank TUI captures | **Argued/owner-routed.** The captures remain blank, but `00-CONTEXT.md` now documents that honestly, identifies the null-stdio root cause, and preserves real audit/sqlite/wire evidence. This is defensible as a scope-owner question, not an implementation-side fabrication. |
| B3 ratified-but-merge-blocked wedge | **Closed, with wording nit N3.** §4.5 now says the witnessed-output sweep is pre-ratification / ratification-candidate work and row 33 remains the merge trigger. |
| B4 decision rows 64/68 + `register_lazy` cite | **Closed, with wording nit N2 outside the row.** Row 64 no longer claims manual validation uses the release stub binary directly. Row 68 uses `LoadPolicy::Lazy { command }`, cites `register_lazy` at `supervisor.rs:370-389`, and distinguishes `ensure_spawned` at `:401-421`. |
| M1 stale panic / `SydPtyNotFound` wording | **Closed.** Phase summaries, defensive negatives, glossary, and row text now use `anyhow::bail!` and `std::process::exit(1)`. |
| M2 §1 hard-requirement disposition overconfident | **Closed.** Requirement #3 is now partial/owner-routed; #1/#2 are closed by the lockin fix. |
| M3 c10 smoke explanation not a proof | **Closed.** The retro explains why c10 missed the supervisor `env_clear()` path and the new lockin regression test fills that gap. |
| N1 post-merge naming | **Closed.** The live framing is pre-merge / ratification-candidate sweep. |
| N2 redundant `--since` enumeration | **Closed.** Row 63 gives the arbitrary `<number><m|h|d>` grammar and examples only as examples. |

## Nits

### N1. Round-3 sibling commit hashes are stale for the branch under review

The current branch landed the fix/capture commits as `7c3c18d` and `c61cc0b`, but the retro cites side-branch equivalents `6f1fe4b` and `de8e187` in the round-3 banner, Phase C text, hard-requirement text, and row 62. Those old commits exist, but they are not the ancestors on `agents/m6/retro-pi`. Replace the stale hashes with the landed hashes before finalizing the append-only row text.

### N2. §6 still has one stale manifest-shape sentence

Row 68 is corrected, but §6 still says Stream F's existing manifest field is `load.triggers.kind = "tool"`. Live Stream F documents `load.command = [...]` / table-form `command`, and row 68 already says so. Change that §6 bullet to `load.command` / `LoadPolicy::Lazy { command }` so the B4 closure is global.

### N3. Final verdict sentence slightly overstates "ready now"

§4.5 correctly says macOS URL, Homebrew smoke, LiteLLM/manual fills, etc. are pre-ratification witnessed gates. The final paragraph says that if owner picks option A, the retro is "ready to RATIFY now". Tighten that to "ready after the §4.5 pre-merge ratification-candidate sweep is filled" (unless those outputs have already landed) to avoid reintroducing the round-2 wedge in miniature.

## Checks performed

- Ran `cd lockin && cargo test -p lockin --test fake_syd_records_cargo_bin_exe_env_after_env_clear --features test-fixture -- fake_syd_records_cargo_bin_exe_env_after_env_clear --nocapture` — pass.
- Checked `lockin/crates/sandbox/src/lib.rs` and `src/tokio.rs` for env re-application before all spawn/status/output paths.
- Checked `transcripts/section-5/00-CONTEXT.md`, `04-audit.txt`, `05-sqlite-audit.txt`, `06-sqlite-entries.txt`, and `wire-events.txt` for live audit/sqlite/wire shape.
- Checked `rafaello/crates/rafaello-core/src/frontend/mod.rs:203-205` for the null-stdio root cause.
- Checked rows 59-68 against live package layout, audit parser, stub exit behavior, and lazy-load supervisor/gate code.
