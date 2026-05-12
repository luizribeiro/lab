# m6 retrospective.md round-2 pi review

> Verdict: blocking
> Counts: B/4 M/3 N/2

Round 2 fixes the round-1 fabricated audit transcript problem in the narrow sense: `04-audit.txt`, `05-sqlite-audit.txt`, `06-sqlite-entries.txt`, and `wire-events.txt` now look like real live-output artifacts and no longer contain invented audit kinds. But the revision also surfaces two load-bearing open issues that cannot be waved past the final v1 gate: the TUI transcript is still blank, and the cold-start flow needed an uncommitted manifest workaround because `env_clear()` strips the syd-pty env that m6 was supposed to fix.

I do not think m6 can ratify with those routed to “post-retro” or “v0.1.1”. Scope made first-chat-from-cold, right-layer syd-pty discovery, and a real tmux-driven recording hard requirements; this retrospective is the last gate before `rafaello-v0.1 → main`.

## Blockers

### B1. The newly discovered `env_clear()` regression directly breaks hard requirements #1/#2

`00-CONTEXT.md` says the recapture only reached the stub because the bundled `rfl-openai` and `rfl-mailcat` manifests had `CARGO_BIN_EXE_syd-pty` added to `[capabilities.default.env].pass` as a workaround (`00-CONTEXT.md:27-36`). Live code shows why: lockin sets `CARGO_BIN_EXE_syd-pty` on the syd child (`lockin/crates/sandbox/src/linux.rs:30-31`), then rafaello clears the sandbox command env (`rafaello-core/src/supervisor.rs:682-700`). The shipped bundled manifests do **not** pass that var (`rafaello-openai/rafaello.toml:29-31`; mailcat has no env pass block).

That is not “only custom manifests” (retrospective.md:1151-1155). The capture required modifying the very two bundled plugins in the canonical m6 cold-start demo. Scope hard requirement #1 says first chat from cold must work with no manual `CARGO_BIN_EXE_syd-pty` workaround, and #2 says the syd-pty problem is solved at the right layer. This must be fixed before retro ratification / merge, not routed to v0.1.1.

### B2. Hard requirement #3 is still not met: the tmux render captures are blank

The recapture honestly documents the blank captures, but that means B1 from round 1 is only partially closed. The three TUI transcript files are blank (`01-after-launch.txt`, `02-modal.txt`, `03-response.txt`), and the retrospective routes the attached-tmux render to a later sweep (retrospective.md:8-21, 540, 575, 1140-1146).

Scope hard requirement #3 required `manual-validation.md` §5 to capture a **real tmux-driven interactive `rfl chat` recording** and explicitly rejected “mechanical coverage in lieu of recording” (scope.md hard requirements; acceptance at scope.md:1876-1879). Wire events + audit rows are useful supporting evidence, but they are not the rendered modal/response transcript that §J2 required. Capture it in an attached tmux session before ratification, or obtain an explicit owner re-ratification changing the hard requirement.

### B3. Round 2 invents a “retro ratified, merge still blocked” state that contradicts row 33 / the task framing

Section 4.5 says the pending manual gates “do **not** block retro ratification” but do block the “m6 RATIFIED → v0.1 → main merge trigger” (retrospective.md:580-585). Section 9 still says m6 retrospective RATIFIED **is** the trigger for the ff-only merge (retrospective.md:1112-1123), and scope acceptance makes macOS CI, clean Homebrew, LiteLLM/bootstrap evidence, and §1-§7+§G operator evidence part of the m6 completion gate (scope.md:1847-1883).

The retrospective can distinguish implementation scaffolding from witnessed evidence (good), but it cannot declare convergence while leaving those witnessed gates pending unless the owner explicitly changes row 33 / scope acceptance. The current text creates false process archaeology around the last gate before merge.

### B4. Proposed decision rows still contain append-only inaccuracies

Most round-1 B3 fixes landed, but two row texts are still not safe to append:

- **Row 68** still starts with the non-live manifest shape “Manifests carrying `[load.triggers]` with `kind = "tool"`” (retrospective.md:961-965). Live m6 is `LoadPolicy::Lazy { command }`; Stream F documents `load.command = [...]` / table-form `command`, not `[load.triggers] kind = "tool"` (`streams/f-manifest/rfc-manifest-schema.md:272-280`, `:318-325`). The same row also cites `register_lazy` at `supervisor.rs:401-421` (retrospective.md:982-985), but live `register_lazy` is `supervisor.rs:370-389`; `401-421` is `ensure_spawned`.
- **Row 64** says the `manual-validation.md` §5 scripted demo consumes the runnable release-tree `rfl-openai-stub` binary (retrospective.md:882-887). The recapture context says the on-disk stub’s 5s timeout was impractical and the run used a long-lived Python `http.server` stub instead (`00-CONTEXT.md:14-25`). Either narrow row 64 to “binary is buildable and the integration test consumes/anchors it” or make the manual validation actually use it.

## Major

### M1. Old false syd-pty / stub wording remains outside the corrected rows

The decision rows were corrected, but the phase summaries and glossary still carry stale language: c09 still says hard `Err(SandboxError::SydPtyNotFound)` (retrospective.md:276-281), c15 says “exhaustion-panic” (retrospective.md:312-315), the defensive negative list names `rfl_openai_stub_scripted_turns_panics_on_exhaustion` (retrospective.md:558), and the glossary entry still says “Deterministic-panic” (retrospective.md:1053-1058). These should be aligned to live `anyhow::bail!` and `std::process::exit(1)` before the drift rows are copied.

### M2. The hard-requirement disposition at §1 is now overconfident

The top-level disposition still marks first-chat-from-cold, syd-pty discovery, and the tmux recording as ✓ (retrospective.md:169-200). Round-2 evidence now says all three have unresolved caveats: env_clear required a workaround, TUI render is blank, and LiteLLM/live bootstrap remains pending. Mirror the §4.5 partial-state language in §1 so the headline does not overclaim.

### M3. The c10 smoke-test explanation is not yet a proof

`00-CONTEXT.md` says the c10 rafaello-side smoke still passes “because … or …; investigation pending” (`00-CONTEXT.md:120-126`). The retrospective repeats that as a reason the env_clear gap is not a Phase C regression (retrospective.md:1151-1155). That is not adversarial enough: either explain exactly why the test misses the canonical bundled-provider/mailcat path, or add a regression test that fails before the env_clear fix.

## Nits

### N1. Use “pre-merge/ratification-candidate sweep”, not “post-merge”

Several places say “post-merge driver sweep” for work that must happen before the ff-only merge (retrospective.md:16, 26, 94, 629, 1143). If the intent is “after retro draft, before merge”, name it that way.

### N2. Row 63 still has a redundant `--since` mini-enumeration

Row 63 now correctly documents arbitrary `<number><m|h|d>`, but it still first lists `--since` as `1h`/`30m`/`24h` (retrospective.md:852-866). Make those examples only once to avoid reintroducing the round-1 ambiguity.

## Round-1 follow-up table

| Prior finding | Round-2 status |
|---|---|
| B1 fabricated §5 transcripts | Partially fixed: audit/sqlite/wire artifacts look live and fabricated audit kinds are gone; still blocking because TUI captures are blank and routed later. |
| B2 acceptance overclaim | Partially fixed: §4 now distinguishes scaffolding vs witnessed evidence; still blocking because it claims pending witnessed gates do not block retro ratification despite row 33 / scope. |
| B3 rows 62/64/65/68 falsehoods | Mostly fixed for rows 62/65 and the main row-64 exhaustion semantics; still blocking for row 68’s `[load.triggers]`/bad line cite and row 64’s “manual validation consumes release stub” overclaim. |
| M1 row 67 wrong files | Closed. Row 67 cites the live non-test allow files. |
| M2 row 63 `--since` grammar | Closed substantively; wording nit remains. |
| M3 missing overview §4.6 `RFL_SPAWN_TRACE_LOG` patch | Closed. Planned overview patch now includes §4.6. |
| M4 I1 filename framing | Closed. The deviation is now correctly framed as crate/process + assertion shape. |
| N1 non-live audit kinds in manual-validation/transcripts | Closed in the recaptured audit artifacts and §6 prose; stale “panic” naming remains separately under M1. |
| N2 row 66 tap distinction | Closed. Row 66 now distinguishes committed formula/automation from owner tap publication. |

## What's working

- The recaptured `04-audit.txt` and sqlite dumps use live audit kinds and live `rfl audit` bracketed request-id formatting.
- Rows 62, 65, 66, and 67 are much closer to appendable architecture text.
- The I1 deviation is now honestly framed.
- `wire-events.txt` is useful supplementary proof for the bus-flow half of the demo; it just cannot replace the required rendered tmux transcript.
