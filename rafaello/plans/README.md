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
3. The milestone branch merges to main with linear history.

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
>    merge the milestone branch to main with linear history. Ping the
>    owner.
>
> Operational guardrails:
>
> - Tmux session names: `rafaello-m<N>-<role>` for design/review work
>   and `rafaello-m<N>-c<NN>-claude` for per-commit work. Never reuse a
>   name. Never touch a session you did not create.
> - One git worktree per agent under `~/lab-wt/<session>`. Branches
>   under `agents/m<N>/...`.
> - All merges via rebase. No merge commits. No force pushes.
> - Pre-commit hooks must pass — never `--no-verify`. If a hook fails
>   on pre-existing breakage, surface to the owner; don't drive-by fix
>   unrelated code.
> - Architectural code stays in the per-commit agent; the milestone
>   driver only orchestrates and merges.
> - When in doubt, save state and ping the owner.

## Tooling notes

- `LITELLM_API_KEY` for the model endpoint at
  `https://litellm.thepromisedlan.club/v1`.
- Default model for rafaello runtime: `vllm/qwen3.6-27b`.
- pi runs at `gpt-5.5` and is not modified by rafaello tooling.
- Rust toolchain pinned via repo-root `rust-toolchain.toml`.
- Pre-commit hooks: rustfmt + clippy + nixpkgs-fmt + statix + deadnix.
