# rafaello m6 — driver pre-flight notes

Driver: claude (worktree `/home/luiz/lab-wt/m6-driver` on `agents/m6/driver`,
forked off `rafaello-v0.1` at `b989fb2`). Date: 2026-05-12.

## m6 in one paragraph

m6 is **v1 polish + release readiness** — the last milestone on the
`rafaello-v0.1` integration branch. On m6's RATIFIED close, `rafaello-v0.1`
merges to `main` per `decisions.md` row 33 and v1 is demo-ready. Roadmap row
asks for: coverage gaps closed, docs pass on `rafaello/README.md` +
`CONTRIBUTING.md`, Homebrew formula, `nix build .#rafaello` green on Linux +
macOS, and a manual end-to-end transcript covering
`init → install rfl-openai → install one tool → chat → tool call with
confirmation → response render → session persist`. The owner-set hard
requirements on top of the roadmap (inlined into the driver prompt
2026-05-12): cold-start first-chat just works, syd-pty discovery is fixed
at the right layer, manual-validation §5 captures a real tmux-driven
interactive recording (not "mechanical-coverage in lieu of"), bootstrap UX
fits in ≤5 lines of shell, and the syd-pty failure-mode is documented in
the m6 retrospective for posterity.

## What the codebase has today

- `rfl chat`, `rfl install`, `rfl status` — implemented.
- `rfl init` — **does NOT exist**. Has to ship in m6.
- `rfl audit` — **does NOT exist**. m5b §5 row 8 routed it to m6.
- `flake.nix` at repo root — exists, but **no `.#rafaello` output**.
  Today the working invocation is `nix develop .#rafaello --command …`
  (devshell), not `nix build .#rafaello` (package). m6 must add the
  package output.
- Homebrew formula — does not exist.
- `rafaello/README.md` (1.3 KB) and `CONTRIBUTING.md` (~1 KB) — exist as
  placeholders.

## Hard requirement #1: the syd-pty wall

Owner hit this on 2026-05-12 against the m5a-RATIFIED build. Summary
captured in the driver prompt:

- `syd` resolves via `LOCKIN_SYD_PATH` (devenv).
- `syd` spawns the plugin under `setup_pty`, which needs `syd-pty`
  adjacent on PATH or `CARGO_BIN_EXE_syd-pty` set; direnv/nix-develop
  strip env vars not allowlisted, so naive `export` doesn't propagate.
- syd's three fixes: PATH, `CARGO_BIN_EXE_syd-pty`, or `sandbox/pty:off`.
- Candidate landing layers (m6 to pick): `rafaello/nix/devenv.nix`
  exports `CARGO_BIN_EXE_syd-pty` (mirroring how it exports
  `LOCKIN_SYD_PATH`); lockin wrapper resolves `syd-pty` adjacent to `syd`
  and sets the env var on the syd child command; or both (belt-and-braces).

**Pre-flight read**: defaulting to *both* is the conservative choice.
Devenv export covers interactive `rfl chat` in the devshell; lockin
wrapper covers any future caller that didn't enter the devshell first
(e.g. eventually a Homebrew-installed `rfl`). The cost of doing both is
one env-var line in `devenv.nix` plus a few lines in `lockin`'s child-
command setup — well under the cost of getting it wrong. Will draft
scope.md proposing both, let pi push back if I'm overpaying.

## Carryovers from m4/m5a/m5b retrospectives

m4 §5 (deferred-to-cleanup):
- Workspace-wide `#[allow(clippy::result_large_err)]` sweep on
  `ReemitError` + `AgentLoopError`.

m5a §5 (routed to m6):
- 6: `#[allow(result_large_err)]` sweep (same as m4 carryover).
- 8: `load.triggers.kind = "tool"` lazy-load not exercised.
- 9: macOS CI green hard gate.
- 10: interactive `rfl chat` recording for `manual-validation.md` §1
  (post-merge sweep).
- 11: `manual-validation.md` skeleton fill.
- 12/13/14/15: defence-in-depth regression anchor tests
  (`rfl_chat_eager_spawns_five_tree_…`,
  `rfl_chat_spawns_inactive_provider_but_reemit_ignores_it`,
  `core_tools_list_registered_before_provider_spawn`, positive
  gate-through-orchestration assertion).

m5b §5 (routed to m6):
- 1: **Multi-turn `rfl-openai-stub` shape** for scripted-turns walkthrough
  in the m6 interactive demo. Load-bearing for the §G transcript.
- 8: **`rfl audit` read CLI** — new subcommand reading the `audit_events`
  table. Owner explicitly wants this for the v1 demo.
- 9: macOS CI green (same as m5a row 9).
- 10: interactive `rfl chat` recording (same as m5a row 10).
- 11: `manual-validation.md` additions (audit-log dumps + macOS-CI URL).
- 12: `core_tools_list_registered_before_provider_spawn.rs` (same as
  m5a row 14).
- 13: `#[allow(result_large_err)]` sweep (same as m4/m5a carryover).

Streams RFC drift: per `milestones/README.md` plus the m1/m3 retros,
all known stream drift was patched in those retros (security RFC in
m1, manifest RFC in m1, renderer RFC in m3, fittings RFCs already
clean). Nothing new to patch in m6 unless implementation surfaces
new drift.

## Likely phase shape (draft input to scope.md)

- **Phase A — `rfl init`**: materialise default `rafaello.lock`,
  pre-install `rfl-openai`, dev-env defaults (LiteLLM endpoint +
  `LITELLM_API_KEY` env-var name). Resolves m4/m5a manual-validation §5
  cold-start.
- **Phase B — `rfl install <plugin>`**: ergonomics polish on existing
  command, trifecta refusal grant-acknowledgement (m5a §Tr precedent).
- **Phase C — syd-pty discovery fix**: devenv export + lockin wrapper
  resolve-adjacent. Per hard requirement #2.
- **Phase D — `rfl audit` read CLI**: from m5b §5 row 8.
- **Phase E — multi-turn `rfl-openai-stub`**: from m5b §5 row 1. Load-
  bearing for §G's scripted-turn interactive recording.
- **Phase F — `nix build .#rafaello`** package output in `flake.nix`,
  green on Linux + macOS. Resolves m5a/m5b §5 macOS-CI-green item.
- **Phase G — Homebrew formula**: per roadmap row.
- **Phase H — README + CONTRIBUTING pass**: 5-line bootstrap snippet
  (hard requirement #4), syd-pty failure-mode documentation (hard
  requirement #5).
- **Phase I — Coverage / regression anchors**: the deferred tests
  (m5a rows 12/13/14/15, m5b row 12), `#[allow(result_large_err)]`
  sweep (m4/m5a/m5b).
- **Phase J — Manual-validation transcript**: tmux-driven `rfl chat`
  capture per hard requirement #3 + the m5a/m5b §5 row-10 carryover.
  This IS the canonical proof-of-life for v1 demo readiness.

## Single milestone vs split (m6a / m6b)

Surface inventory (rough):
- new commands: `rfl init`, `rfl audit` (2)
- syd-pty discovery fix at two layers (2)
- multi-turn provider stub (1, with tests)
- `nix build` package output + CI matrix coverage (2-3)
- Homebrew formula (1-2)
- README + CONTRIBUTING + syd-pty doc (1-2)
- regression-anchor tests + clippy sweep (3-5)
- manual-validation transcript (1, but it's the demo)

Rough sizing: 18–28 commits. m5b (a smaller surface than this) was
17 commits with 6 pi rounds. m1 was the canary at ~30+ commits.
m3/m4 were each ~30+.

**Recommendation for scope.md round 1**: propose a *single* m6, with
the phase ordering above. Rationale: the demo bar is the integrated
flow (init → install → chat → tool-call → confirm → response → persist);
splitting would force the headline demo into m6b's retrospective only,
and the headline demo is what v1-demo-readiness IS. The cohesion cost
of a split exceeds the wall-clock cost of one longer milestone.

If pi pushes back during round 1 with a credible "this is m5-sized
again", a clean fold-line would be:
- **m6a (ergonomics)**: Phases A, B, C, D, E, I, J — first-chat works
  cold, audit CLI lands, transcript captured, regression anchors and
  clippy sweep done. Demo bar: cold-start first-chat against the
  configured LiteLLM endpoint produces a tool call + confirm + response.
- **m6b (release)**: Phases F, G, H — nix build green on both
  platforms, Homebrew, README/CONTRIBUTING docs pass. Demo bar:
  `nix build .#rafaello` works on Linux + macOS and `brew install`
  works in a clean macOS shell.

I'll lead with the single-milestone proposal in scope.md round 1 and
flag this fold-line as the fallback for pi to consider — owner can
weigh in if pi convergence forces the question.

## Operational guardrails (carried)

- Tmux session names: `rafaello-m6-<role>` and `rafaello-m6-c<NN>-claude`.
- Worktrees under `/home/luiz/lab-wt/m6-*`. Branches under `agents/m6/...`.
- Pre-commit hooks pass; `PREK_ALLOW_NO_CONFIG=1` for orchestrator
  commits in worktrees that lack the symlink.
- `tmux-wait` race: sleep 10–15s after `send-keys` before invoking
  the wait helper.
- Disk hygiene: `rm -rf` agent worktree `rafaello/target` after each
  ff-merge.
- Premature self-CONVERGED: do not self-claim CONVERGED until pi
  explicitly returns 0/0/0 (m5b made this mistake twice).
- m6 introduces a system dependency layer change (syd-pty discovery)
  — push to CI inside the milestone, not at retrospective time
  (m2 §5.7 lesson).

## Next steps

1. Commit this pre-flight (owner sanity check on next status pass).
2. Spawn `rafaello-m6-scope-claude` + `rafaello-m6-scope-pi` to draft
   and review `scope.md` round 1 against the phase shape above. Hand
   them: this pre-flight, the m6 roadmap row, the hard requirements,
   the m4/m5a/m5b §5 carryovers, and the streams RFCs.
3. Iterate to convergence (expect 4–6 pi rounds at this surface area).
4. Ping owner for ratification.
