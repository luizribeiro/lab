# m0-fittings — driver notes

> **Status:** kickoff guide for the milestone driver picking up
> Phase 3. Both `scope.md` and `commits.md` are owner-ratified
> (2026-05-08). This doc is the operational manual for walking
> `commits.md` from c01 to c32.

## What you (the driver) need to read first

In order:

1. `rafaello/plans/README.md` — workflow, conventions, **Recurring
   operational gotchas** (load-bearing — don't skip).
2. `rafaello/plans/overview.md` — v1 architecture, especially §15
   (cross-stream gaps) and §16 (v1 scope cut).
3. `rafaello/plans/decisions.md` — 34 ratified rows; rows 18, 22,
   23, 33, 34 are the load-bearing fittings calls.
4. `rafaello/plans/glossary.md` — spot-check for any term you'd
   otherwise have to derive.
5. `rafaello/plans/streams/b-fittings/rfc-fittings-notifications.md`
   and `rfc-fittings-errors.md` — the source RFCs that scope+commits
   derive from.
6. `rafaello/plans/milestones/m0-fittings/scope.md` — what m0 ships.
7. `rafaello/plans/milestones/m0-fittings/commits.md` — your work
   queue, c01 through c32.

The four pi reviews under `m0-fittings/` (`pi-review-1..3.md` for
scope, `commits-pi-review-1..3.md` for commits) are useful when you
need the *why* behind a specific decision in scope or commits.

## Branch model

- Integration branch: `rafaello-v0.1`. Already created off
  `rafaello-design` ahead of this kickoff.
- Per-commit agent branches: `agents/m0/c<NN>` (zero-padded), each
  branched off the *current* tip of `rafaello-v0.1` (not a stale
  base — pi review-1 finding 2 was about exactly this category of
  bug).
- After each per-commit agent finishes, the milestone driver
  verifies the commit landed clean, rebases the agent branch onto
  `rafaello-v0.1`, and ff-merges. No merge commits, no force pushes.
- Final merge of `rafaello-v0.1` into `main` happens at v1, not at
  m0 end (per `decisions.md` row 33).

## Per-commit pattern

For each commit row in `commits.md`, in order:

1. **Set up the worktree.**
   ```sh
   git worktree add /home/luiz/lab-wt/m0-c<NN> \
       -b agents/m0/c<NN> rafaello-v0.1
   ```

2. **Spawn a tmux session in that worktree.**
   ```sh
   tmux new-session -d -s rafaello-m0-c<NN>-claude \
       -x 220 -y 50 -c /home/luiz/lab-wt/m0-c<NN>
   tmux send-keys -t rafaello-m0-c<NN>-claude \
       "claude --dangerously-skip-permissions" Enter
   sleep 2
   tmux-wait rafaello-m0-c<NN>-claude "esc to interrupt" 60
   ```

3. **Send the per-commit briefing prompt.** Template below.

4. **Wait for the agent to finish.** Per `plans/README.md`'s
   gotcha section: sleep 10–15s before tmux-wait, and verify the
   commit actually landed via `git log -1` in the worktree
   afterward.

5. **Verify acceptance.** Read the commit's "Acceptance" bullet in
   `commits.md`. Confirm:
   - Pre-commit hooks passed (the hook output is in the agent's
     pane).
   - The named tests in the acceptance row exist and pass under
     `cargo test` from `fittings/` (or `mcp-server/` for c26+).
   - For Group 5 commits, confirm `npm run check:real-client`
     hasn't regressed.

6. **Merge into rafaello-v0.1.**
   ```sh
   git checkout rafaello-v0.1
   git merge --ff-only agents/m0/c<NN>
   ```
   If the merge isn't a fast-forward, the agent branched from a
   stale base — rebase first, re-verify, then merge. (This should
   not happen if step 1 used the current tip.)

7. **Tear down.**
   ```sh
   tmux kill-session -t rafaello-m0-c<NN>-claude
   git worktree remove /home/luiz/lab-wt/m0-c<NN>
   git worktree prune
   ```
   Keep the agent branch as a ref for now; it gets pruned post-m0.

8. **Move on to the next commit.**

## Per-commit prompt template

Paste this verbatim, substituting `<NN>` for the commit number and
`<row>` for the entire commit row from `commits.md` (Subject + What
+ Why + Depends on + Acceptance):

```
You're implementing commit c<NN> for rafaello milestone m0-fittings.

Working environment:
- Worktree: /home/luiz/lab-wt/m0-c<NN>
- Branch: agents/m0/c<NN> (already created off rafaello-v0.1)
- Single-commit scope: this session lands ONE commit, then ends.

Read first:
- rafaello/plans/milestones/m0-fittings/scope.md (the m0 scope; ratified)
- rafaello/plans/milestones/m0-fittings/commits.md (your full milestone context)
- The relevant streams/b-fittings/ RFC sections referenced by your commit row
- The current state of any source files this commit touches

Your commit row from commits.md:
<row>

Land exactly this commit. The acceptance criteria define "done":
- New tests live under fittings/tests/ (or wherever the row says)
- Tests land WITH the implementation in this same commit per
  ~/.claude/CLAUDE.md
- Pre-commit hooks (rustfmt + clippy + cargo test) MUST pass; never
  --no-verify. If a hook fails on pre-existing unrelated breakage,
  stop and ping the milestone driver
- Subject style: <type>(<scope>): <imperative>, matching the row
- One commit, one idea — do not bundle in unrelated cleanup

When the commit has landed cleanly, end with the exact phrase on
its own line: COMMIT LANDED
```

The `COMMIT LANDED` sentinel lets you `tmux-wait` for completion
(after the standard sleep), then verify via `git log -1`.

## After c32 lands

1. **Retrospective.** Spawn claude+pi in tmux for
   `retrospective.md`:
   - Claude reviews the m0 diff against `scope.md` and
     `commits.md`, writes a draft retrospective answering:
     - Did anything in m0's implementation invalidate `overview.md`,
       `decisions.md`, or any stream RFC?
     - What slipped scope or got cut during implementation?
     - Coverage: every named test in scope's matrix actually landed?
   - Pi reviews adversarially.
   - Owner ratifies on convergence.
2. **Apply any retrospective updates** to `overview.md`,
   `decisions.md`, or stream RFCs as commits on `rafaello-v0.1`
   (not separate branches for these small docs-only deltas).
3. **Ping the owner.** m0 is done; m1 scoping starts when the owner
   says go.

`rafaello-v0.1` does NOT merge to `main` at m0 end — it accumulates
through m1–m6 and merges at v1 completion (`decisions.md` row 33).

## Group-level checkpoints

Useful waypoints for owner sanity checks (no automatic gate, just
points where it's natural to pause and verify):

- **After c10**: API + notify cutover green. Workspace builds; basic
  notification flow works; bidirectional `peer.call` doesn't yet.
  Reasonable place to take a breath.
- **After c20**: Group 3 done — bidirectional `PeerHandle` + id-null
  semantics. `id_namespace_isolation.rs` and full
  `peerhandle_bidirectional.rs` should pass.
- **After c25**: cancellation primitive complete; Group 4 done.
- **After c29**: mcp-server fully migrated; JS-SDK interop passes
  end-to-end against the rebuilt server.
- **After c32**: m0 demo bar green; `manual-validation.md` written.

If something feels wrong at any checkpoint, the milestone driver
should pause and surface — don't barrel through.

## m0a/m0b split decision

Per `commits.md`'s split-point note: only clean stopping point
inside m0 is **after c10**. Default is to ship m0 as one milestone.
If during implementation it becomes clear Group 3 cannot land
green on top of Group 2 alone, the driver should propose an
m0a (c01–c10) / m0b (c11–c32) split for owner approval at that
point. Otherwise: one milestone, all 32 commits.

## When things go wrong

Cross-references to the gotchas in `plans/README.md`:

- Per-commit agent doesn't land the commit / hangs / claims
  COMMIT LANDED but `git log` says otherwise → see "Verify commits
  actually landed".
- pi prints reviews but doesn't save → see "Pi sometimes prints
  reviews to chat without saving the file".
- `git commit` from orchestrator complains about
  `.pre-commit-config.yaml` not found → see
  "PREK_ALLOW_NO_CONFIG=1 for orchestrator-side commits".
- `tmux-wait` returns immediately → see "tmux-wait race after
  tmux send-keys".

When in genuine doubt, save the state (don't kill running tmux
sessions; commit any in-flight work to the agent branch) and ping
the owner.

## Quick command reference

Prep before starting any commit:
```sh
cd /home/luiz/lab
git checkout rafaello-v0.1
git pull --ff-only          # if applicable
```

Spawn per-commit agent (paste-buffer pattern preserves multi-line
prompts):
```sh
tmux new-session -d -s rafaello-m0-c<NN>-claude \
    -x 220 -y 50 -c /home/luiz/lab-wt/m0-c<NN>
tmux send-keys -t rafaello-m0-c<NN>-claude \
    "claude --dangerously-skip-permissions" Enter
sleep 2
tmux-wait rafaello-m0-c<NN>-claude "esc to interrupt" 60
cat <<'PROMPT' | tmux load-buffer -b m0-c<NN> - && \
    tmux paste-buffer -b m0-c<NN> -t rafaello-m0-c<NN>-claude && \
    tmux send-keys -t rafaello-m0-c<NN>-claude Enter
<paste the per-commit prompt template here, with substitutions>
PROMPT
sleep 15
tmux-wait rafaello-m0-c<NN>-claude "esc to interrupt" 1800
```

Verify + merge:
```sh
cd /home/luiz/lab-wt/m0-c<NN>
git log -1                   # confirm the commit exists
cd /home/luiz/lab
git checkout rafaello-v0.1
git merge --ff-only agents/m0/c<NN>
```

Cleanup:
```sh
tmux kill-session -t rafaello-m0-c<NN>-claude
git worktree remove /home/luiz/lab-wt/m0-c<NN>
git worktree prune
```
