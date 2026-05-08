# rafaello design plans

This directory holds rafaello's design artifacts and milestone plans.
The agent itself is not implemented yet; everything here is paper-first,
by design.

## Layout

- `overview.md` — single source of truth for the v1 architecture.
- `decisions.md` — append-only architecture decision log.
- `glossary.md` — one-line definitions of load-bearing terms.
- `streams/` — detailed RFCs per subsystem (security, fittings,
  scripting, renderer, manifest). Inputs to `overview.md`.
- `milestones/` — roadmap from m0 to v1, one subdirectory per milestone.

## Workflow

### Phase 1 — Design

`overview.md` is the source of truth for v1. Authored by claude+pi
iteratively; the project owner reviews on convergence. The five stream
RFCs are detailed inputs; if a stream RFC conflicts with `overview.md`,
`overview.md` wins.

### Phase 2 — Per-milestone scoping

For each milestone:

1. Claude drafts `scope.md` — what's in, what's deferred, what
   "success" looks like (testable demo).
2. Pi reviews `scope.md`.
3. The project owner ratifies `scope.md`.
4. Claude drafts `commits.md` — ordered list of commits, each with a
   subject, rationale, dependencies, and acceptance criteria. Small
   commits (one commit = one idea), good test coverage, manual
   validation with tmux + litellm when necessary, per the project
   commit guidelines in `~/.claude/CLAUDE.md`.
5. Pi reviews `commits.md`.
6. The project owner ratifies `commits.md`.

### Phase 3 — Implementation

For each commit on `commits.md`, in order:

1. Spawn one claude tmux session in a fresh worktree on
   `agents/m<N>/c<NN>`.
2. Hand the agent: `overview.md`, the relevant `streams/` RFCs,
   `scope.md`, and the specific row from `commits.md`.
3. Wait for the commit to land cleanly (pre-commit hooks pass).
4. Cross it off `commits.md`. Move on.

After the milestone's commits all land:

1. Pi reviews the full milestone diff against `scope.md` and
   `commits.md`. Both review and the diff context land in
   `retrospective.md`.
2. Claude+pi discuss whether anything learned invalidates
   `overview.md`, `decisions.md`, or any stream RFC. Updates land as
   commits on the milestone branch before merge.
3. The milestone branch merges into the long-running integration
   branch `rafaello-v0.1` with linear history (see `decisions.md`
   row 33). `main` stays at the `rafaello-design` merge until v1 is
   demo-ready, at which point `rafaello-v0.1` merges to `main` in one
   pass. Optional exception: m0's fittings work could split out as a
   `fittings-v0.X` merge to `main` ahead of v1 if a fittings consumer
   needs it earlier — call this out in m0's retrospective if it
   applies.

## Authoring conventions

- `decisions.md` is append-only. Reversals add a new row; do not edit
  prior entries.
- Stream RFCs in `streams/` are not retroactively rewritten when
  `overview.md` evolves. Drift gets resolved in `overview.md` and
  called out in the next milestone retrospective.
- Glossary entries are added when a new load-bearing term first lands
  in `overview.md` or a stream RFC.

## How to drive a milestone

A future claude session with `--dangerously-skip-permissions` and
access to the `drive-agents` skill can take over orchestration. Hand it
this prompt:

> You're the milestone driver for rafaello milestone `m<N>`.
>
> 1. Read `rafaello/plans/README.md`, `overview.md`, `decisions.md`,
>    `glossary.md`, every file under `streams/`, and
>    `milestones/m<N>/scope.md` / `commits.md` if they exist.
> 2. If `scope.md` doesn't exist yet: spawn claude+pi tmux sessions per
>    Phase 2 to draft and review it iteratively. Ping the project owner
>    for ratification at convergence.
> 3. If `commits.md` doesn't exist yet: same flow.
> 4. Once both are ratified: walk `commits.md` in order. For each
>    commit, spawn a fresh claude tmux session on a worktree at
>    `agents/m<N>/c<NN>`, hand it the inputs above plus the specific
>    commit row, wait for completion, verify the commit landed clean,
>    move on.
> 5. After the last commit: spawn claude+pi for `retrospective.md`,
>    update `overview.md` / `decisions.md` with anything learned, and
>    merge the milestone branch into `rafaello-v0.1` with linear
>    history (per `decisions.md` row 33; `main` waits for v1).
>    Ping the owner.
>
> Operational guardrails:
>
> - Tmux session names: `rafaello-m<N>-<role>` for design/review work
>   and `rafaello-m<N>-c<NN>-claude` for per-commit work. Never reuse a
>   name. Never touch a session you did not create.
> - One git worktree per agent under `/home/luiz/lab-wt/<session>`.
>   Branches under `agents/m<N>/...`.
> - All merges via rebase. No merge commits. No force pushes.
> - Pre-commit hooks must pass — never `--no-verify`. If a hook fails
>   on pre-existing breakage, surface to the owner; don't drive-by fix
>   unrelated code.
> - Architectural code stays in the per-commit agent; the milestone
>   driver only orchestrates and merges.
> - When in doubt, save state and ping the owner.

## Recurring operational gotchas

These were learned the hard way during the design and m0-scoping
phases. The milestone driver should be aware before starting:

- **`tmux-wait` race after `tmux send-keys`.** The busy pattern
  doesn't appear instantly. After sending a prompt, sleep 10–15s
  before invoking `tmux-wait`; otherwise `tmux-wait` polls before
  the agent has started working and exits immediately on a
  false-negative. Pattern: `sleep 15; tmux-wait <session>
  "Working\.\.\." 1800`. For claude sessions the busy pattern is
  `esc to interrupt`; for pi it's `Working\.\.\.`.
- **Pi sometimes prints reviews to chat without saving the file.**
  Recurring failure mode. After pi finishes, verify the expected
  file landed via `ls` / `wc -l`. If it didn't, send a short nudge
  asking pi to save the FULL review to the specific path and
  commit. If pi writes the file but doesn't commit (also recurring),
  commit from the orchestrator side using `PREK_ALLOW_NO_CONFIG=1`
  (see next item).
- **Pi WebSocket disconnects mid-write.** Pi loses the connection
  to its model occasionally and abandons the in-flight write. If a
  review is partially saved or the pane shows "WebSocket closed",
  send a "retry the save" prompt; pi resumes from its prior context.
- **`PREK_ALLOW_NO_CONFIG=1` for orchestrator-side commits in
  worktrees.** The repo uses a `.pre-commit-config.yaml` symlink
  that resolves only inside the devenv shell. Worktrees outside
  `/home/luiz/lab` (e.g. agent worktrees under `lab-wt/`) don't
  have the symlink, so `git commit` errors with "config file not
  found". Prefix orchestrator-side commits in those worktrees with
  `PREK_ALLOW_NO_CONFIG=1` (skips prek's check; the actual hooks
  are docs-only safe). Per-commit agent sessions running under
  `claude --dangerously-skip-permissions` typically have direnv
  active and don't need this.
- **Cherry-pick range gotcha when consolidating pi reviews back
  to the main branch.** Use `git cherry-pick <main-branch-tip>..<pi-branch-tip>`
  with the *current* tip of the integration branch, not a stale
  base. Otherwise you get duplicate-commit conflicts on commits
  the integration branch already has.
- **Verify commits actually landed.** After every agent-driven
  commit, run `git log --oneline -1` in the worktree to confirm.
  Don't trust "REVISION READY" / "REVIEW SAVED" sentinels on
  their own — they are reliable from claude (which always
  commits) but unreliable from pi (which sometimes prints without
  committing).
- **Don't poll `tmux capture-pane` looking for the sentinel
  string itself.** The orchestrator's prompt to the agent contains
  the sentinel string ("REVISION READY", "REVIEW SAVED", etc.) so
  a raw grep matches before the agent has even started. Use
  `tmux-wait` on the busy pattern, or check for the file landing
  on disk.

## Tooling notes

- `LITELLM_API_KEY` for the model endpoint at
  `https://litellm.thepromisedlan.club/v1`.
- Default model for rafaello runtime: `vllm/qwen3.6-27b`.
- pi runs at `gpt-5.5` and is not modified by rafaello tooling.
- Rust toolchain pinned via repo-root `rust-toolchain.toml`.
- Pre-commit hooks: rustfmt + clippy + nixpkgs-fmt + statix + deadnix.
