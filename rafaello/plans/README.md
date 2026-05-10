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
  called out in the next milestone retrospective. **If you catch
  yourself wanting to edit a stream RFC mid-implementation, stop —
  that's `decisions.md` territory.** The RFC stays as a historical
  artefact of the ratification round.
- Glossary entries are added when a new load-bearing term first lands
  in `overview.md` or a stream RFC.

## How to drive a milestone

A future claude session with `--dangerously-skip-permissions` and
access to the `drive-agents` skill can take over orchestration. Hand it
this prompt:

> You're the milestone driver for rafaello milestone `m<N>`.
>
> 1. Read `rafaello/plans/README.md`, `milestones/README.md`,
>    `overview.md`, `decisions.md`, `glossary.md`, every file
>    under `streams/`, the previous milestone's `retrospective.md`
>    (for any deferred-to-m<N> items), and
>    `milestones/m<N>-<topic>/scope.md` / `commits.md` if they
>    exist. **The milestone's name, topic, one-line goal, and
>    demo are pre-ratified in the `milestones/README.md`
>    roadmap table** — that's where you read what `m<N>` is.
>    The directory name follows `m<N>-<topic>` where
>    `<topic>` is the kebab-case short form of the roadmap
>    row's name (m0-fittings, m1-manifest, m2-broker-spawn, …).
>    Do not improvise the milestone topic from `overview.md` or
>    retrospectives; the roadmap is the source of truth.
> 2. If `scope.md` doesn't exist yet: spawn claude+pi tmux sessions per
>    Phase 2 to draft and review it iteratively. Ping the project owner
>    for ratification at convergence.
> 3. If `commits.md` doesn't exist yet: same flow.
> 4. Once both are ratified: walk `commits.md` in order. For each
>    commit, spawn a fresh claude tmux session on a worktree at
>    `agents/m<N>/c<NN>`. **Inline the full commit row text + every
>    acceptance bullet verbatim into the per-commit prompt** —
>    do NOT cite by row number and tell the agent to read
>    `commits.md` itself. Citing-by-row delegates the granularity
>    decision (and adjacent-row bundling temptation) to the agent;
>    inlining keeps it with the orchestrator. m1 §4.2.
>    Wait for completion, verify the commit landed clean, move on.
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
- **Retrospective worktrees need the same pre-commit symlink
  workaround as agent worktrees.** When the milestone driver
  creates a worktree for the retrospective phase
  (e.g. `/home/luiz/lab-wt/m<N>-retro-claude`), the
  `.pre-commit-config.yaml` symlink isn't there. Either symlink
  it manually from the nix store, or use `PREK_ALLOW_NO_CONFIG=1`
  for orchestrator commits in that worktree (same pattern as the
  agent worktrees gotcha above). m0 retrospective §4.5.
- **`nix develop` invocations need `--impure`.** Manual-validation
  steps like `nix develop .#fittings --command cargo test
  --workspace` fail without `--impure` due to a devenv "current
  directory" assertion. Always invoke `nix develop` with
  `--impure` for these flows. CI already does this. m0
  retrospective §4.6 / `manual-validation.md` §4.
- **Dirty `Cargo.lock` in `/home/luiz/lab` silently aborts
  `git merge --ff-only` from agent branches.** During Phase 3
  the driver may run `cargo metadata` / `cargo doc` against the
  main lab worktree to spot-check things; these update
  `rafaello/Cargo.lock`. The next `git merge --ff-only
  agents/m<N>/c<NN>` then fails with "Please commit your
  changes or stash them before you merge. Aborting" — but the
  output also prints "Updating ..." which makes the failure
  easy to miss when the orchestrator pipes through `tail -2`.
  Mitigation: stash `Cargo.lock` (or any other dirty file)
  before each ff-merge, OR run all driver-side cargo commands
  inside an agent worktree, never in `/home/luiz/lab`. m2
  Phase 3 lost ~10 minutes to this between c02 and c03; m2
  retrospective §4.5.
- **`.pre-commit-config.yaml` symlink missing in worktrees
  outside `/home/luiz/lab`.** The repo's pre-commit config is
  a Nix-store symlink only present in the main worktree.
  Per-commit agent sessions in `/home/luiz/lab-wt/...` hit
  "config file not found" from `prek` and lose 5-15 min
  trying workarounds. Mitigation: symlink it at every
  worktree creation: `ln -s
  /nix/store/.../pre-commit-config.json
  /home/luiz/lab-wt/m<N>-c<NN>/.pre-commit-config.yaml`. m2
  c17 lost 6 minutes to this until the driver added the
  symlink; subsequent commits had it pre-applied. m2
  retrospective §4.6.
- **Pi-as-diagnostic-tool when an agent thrashes.** When a
  per-commit agent has been thrashing for >30 min on what
  looks like a single concrete bug (test hangs, weird API
  mismatch), spawning a parallel pi session in a snapshot
  worktree to root-cause the issue can collapse the loop. m2
  c23: the per-commit agent thrashed for 1h+ on a
  `fixture_publish_one_emits_event` hang; pi root-caused it
  in <30 min (`Value::Null` is invalid `bus.publish` params —
  fittings rejects, publisher panics, observer hangs forever)
  AND landed the fix as a diagnostic commit that the driver
  cherry-picked as the canonical c23. Pattern: copy the
  agent's in-progress files into a fresh `pi`-worktree;
  prompt pi with concrete hypotheses and ask for verdict +
  fix; if pi succeeds, cherry-pick the fix with the canonical
  commit subject. m2 retrospective §4.2.

## Patterns from prior milestones

Lessons learned from m0 (`milestones/m0-fittings/retrospective.md`)
that future milestone drivers should plan around:

- **Workspace-wide cutover commits are unavoidable for breaking
  trait changes.** If a stream RFC requires a breaking trait
  change with multiple in-tree consumers, plan **one consolidated
  cutover commit** in `commits.md` up front. Trying to stage it
  across 2–3 commits fails the per-commit green-bar; pi will reject
  it during `commits.md` review. Document the size in the commit
  body so reviewers know it's intentional. m0 retrospective §4.1
  (concrete example: m0 c08 `feat(fittings): API cutover`).
- **Pi review iterations on `commits.md` are worth the wall-clock
  — expect at least two rounds, plan for three.** m0's commit plan
  went through three pi rounds before ratification: round 1 caught
  the API-cutover-must-be-one-commit issue and a wire-vs-core
  layering bug; round 2 caught a missing `Panic` variant and a
  premature client-side test; round 3 was acceptance-traceability
  cleanup. If `commits.md` looks obviously right after one pi
  pass, that's suspicious. m0 retrospective §4.2.
- **Two-stage tests are the right way to ladder API-surface
  dependencies.** When a single scope-named test depends on
  multiple commits, land the test exercising whatever surface
  exists at the earlier commit, then *extend* it (not duplicate)
  when the rest arrives. m0 examples:
  `peerhandle_outside_handler.rs` (c10 server, c19 client),
  `bounded_notify_drop.rs` (c09 base, c14 post-flood `peer.call`).
  The trap to avoid: "punt the whole test to the last commit" —
  that grows the late commit out of its size budget and makes the
  early commits look untested. m0 retrospective §4.3.
- **Retrospective drafts deserve the same adversarial review as
  scope and commits.** m0's first-pass retrospective was
  overconfident in three places ("no coverage gaps", "no overview
  drift", an invented `originalCode` claim) and pi review-1 caught
  all three. m1 needed **four** rounds on retrospective.md before
  ratification — round 1 incorrectly punted Stream A drift
  patches, round 2 caught lockin.toml + stale fixtures drift,
  round 3 caught helper drift, round 4 caught a self-declared
  fittings-flake waiver that needed explicit owner approval.
  Always run `retrospective.md` through pi before owner sign-off;
  budget for at least 2 rounds and up to 4 if the milestone has
  meaningful drift. m0 `retrospective-pi-review.md` /
  `retrospective-pi-review-2.md`; m1
  `retrospective-pi-review.md` through `retrospective-pi-review-4.md`.
- **Per-commit agent prompts must inline the row text and every
  acceptance bullet, not cite by row number.** m1's plan c31 +
  c32 bundled into a single git commit (`14a1688`) because the
  per-commit prompt cited "the next unlanded row" rather than
  inlining the c31 row text + acceptance. The c31 agent opened
  `commits.md`, saw c31 and c32 sitting adjacent, judged the
  increment small, and bundled. Bundling itself was defensible —
  but the *granularity decision happened inside the agent*, not
  in the orchestrator's prompt. Cite-by-row delegates a
  scope-vs-implementation trade-off the orchestrator should own;
  inlining makes the result predictable. Also defends against
  mid-implementation `commits.md` drift: an in-flight per-commit
  agent that re-reads the doc fresh would see different text
  than prompt-time text. m1 §4.2 + §4.5.
- **Surface size predicts pi-rounds, not "applying ratified RFCs."**
  m1 was supposed to be smaller than m0 ("apply Stream A + Stream
  F to a concrete commit plan"). Reality: m1's 6k-LoC new crate
  with V1+V2+V3 dual-validation mirroring took **6 pi rounds on
  scope, 3 on commits, 4 on retrospective**, vs m0's 3+3+2.
  m2 (broker + locked plugin spawn) pushed it further: **8 rounds
  on scope (one was a runtime-extensibility sanity check, preserved
  on disk), 4 on commits, 2 on retrospective**. When sizing a
  future milestone, weight by surface area honestly: a brand-new
  crate with dual-side validation needs more rounds than
  refactoring an existing crate, even when the design is
  ratified. m1 §4.1, m2 §4.8.
- **Synthetic-stub tests need a planned successor in `commits.md`.**
  When a `commits.md` row stages a test against a synthetic
  failure path baked into an in-progress commit (e.g. "this stub
  returns `Err`; verify unwind"), the row that *removes the stub*
  must either (a) name a fault-injection mechanism the test will
  pivot to, or (b) include an explicit deletion rationale in its
  acceptance lines. Without that, the stub-removal commit silently
  deletes the synthetic test (because the synthetic `Err` is
  gone) and the underlying real failure path goes uncovered.
  m2's c19 staged three unwind tests against a Phase-B-not-yet-
  implemented stub; c21 finalised Phase B and deleted all three
  rather than rewriting against a fault-injection mechanism that
  didn't exist yet. The two unwind windows (post-register and
  pre-register/post-socketpair) are now uncovered in m2 and
  filed as the m3 §5.1 follow-up. m2 §3.3 + §4.1.
- **Local `nix develop` aggregates more than the CI devshell —
  push to CI early when introducing system dependencies.** The
  default `.#default` devshell may include exports from
  neighbouring projects' `devenv.nix` files (e.g. the lockin
  shell's `LOCKIN_SYD_PATH`), but CI explicitly enters
  `.#<project>`'s own devshell and only sees that project's
  exports. m2 round-1 CI failed on Linux because
  `rafaello/nix/devenv.nix` didn't export `LOCKIN_SYD_PATH`
  even though the local shell did, and on macOS because a
  `SOCK_CLOEXEC` cfg-gate was missing — both invisible locally.
  When a milestone introduces a new system dependency (a syd
  enforcer, a socket flag, a platform-specific syscall), push
  to CI inside the milestone rather than waiting until the
  retrospective; the round-trip cost of catching it locally
  is much cheaper. m2 §5.7.

## Tooling notes

- The dev environment's OpenAI-compatible endpoint is a LiteLLM
  proxy at `https://litellm.thepromisedlan.club/v1`, with the
  API key in env var `LITELLM_API_KEY`. **This is deployment
  configuration, not a baked-in dependency**: the bundled
  default provider plugin (`rfl-openai`, `decisions.md` row 38)
  speaks the OpenAI Chat Completions wire protocol and works
  against any compatible endpoint (OpenAI's API directly, vLLM,
  a local stub, etc.); `rfl init` materialises the dev-environment
  values into `rafaello.lock` only because that's what the dev
  setup uses.
- Default model for rafaello runtime: `vllm/qwen3.6-27b`.
- pi runs at `gpt-5.5` and is not modified by rafaello tooling.
- Rust toolchain pinned via repo-root `rust-toolchain.toml`.
- Pre-commit hooks: rustfmt + clippy + nixpkgs-fmt + statix + deadnix.
