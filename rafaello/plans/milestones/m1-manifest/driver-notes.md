# m1-manifest — driver notes

> **Status:** kickoff guide for the milestone driver picking up
> Phase 3. Both `scope.md` and `commits.md` are owner-ratified
> (2026-05-08). This doc is the operational manual for walking
> `commits.md` from c01 to c37.

## What you (the driver) need to read first

In order:

1. `rafaello/plans/README.md` — workflow, conventions,
   **Recurring operational gotchas** (load-bearing — don't skip),
   **Patterns from prior milestones** (m0 lessons that apply).
2. `rafaello/plans/overview.md` — v1 architecture; for m1, §3,
   §4 (esp. §4.2–§4.5), §5, §6, §15.1, §16 are the load-bearing
   sections.
3. `rafaello/plans/decisions.md` — for m1, rows **5, 12, 17,
   25, 26, 27, 30, 31, 32, 36** are load-bearing.
4. `rafaello/plans/glossary.md` — spot-check for any term you'd
   otherwise have to derive.
5. `rafaello/plans/streams/f-manifest/rfc-manifest-schema.md`
   end to end + `rafaello/plans/streams/a-security/rfc-security-model.md`
   §3, §5, §6, §7 — the source RFCs.
6. `rafaello/plans/milestones/m1-manifest/scope.md` — what m1
   ships.
7. `rafaello/plans/milestones/m1-manifest/commits.md` — your
   work queue, c01 through c37.

The six pi reviews under `m1-manifest/` (`pi-review-1.md`
through `pi-review-6.md` for scope, `commits-pi-review-1.md`
through `commits-pi-review-3.md` for commits) are useful when
you need the *why* behind a specific decision. The §"What
changed from prior drafts" sections in scope.md and commits.md
map findings to resolutions.

## Branch model

- Integration branch: `rafaello-v0.1`. Already accumulated m0;
  m1 commits land on top.
- Per-commit agent branches: `agents/m1/c<NN>` (zero-padded),
  each branched off the *current* tip of `rafaello-v0.1` (not a
  stale base — m0 retrospective gotcha "Cherry-pick range
  gotcha when consolidating pi reviews back to the main branch").
- After each per-commit agent finishes, the milestone driver
  verifies the commit landed clean, then ff-merges into
  `rafaello-v0.1`. No merge commits, no force pushes.
- Final merge of `rafaello-v0.1` into `main` happens at v1, not
  at m1 end (per `decisions.md` row 33).

## Per-commit pattern

For each commit row in `commits.md`, in order:

1. **Set up the worktree.**
   ```sh
   git worktree add /home/luiz/lab-wt/m1-c<NN> \
       -b agents/m1/c<NN> rafaello-v0.1
   ```

2. **Spawn a tmux session in that worktree.**
   ```sh
   tmux new-session -d -s rafaello-m1-c<NN>-claude \
       -x 220 -y 50 -c /home/luiz/lab-wt/m1-c<NN>
   tmux send-keys -t rafaello-m1-c<NN>-claude \
       "claude --dangerously-skip-permissions" Enter
   sleep 2
   tmux-wait rafaello-m1-c<NN>-claude "esc to interrupt" 60
   ```

3. **Send the per-commit briefing prompt.** Template below.

4. **Wait for the agent to finish.** Per `plans/README.md`'s
   gotcha section: sleep 15 s before tmux-wait, and verify the
   commit actually landed via `git log -1` in the worktree
   afterward.

5. **Verify acceptance.** Read the commit's "Acceptance" bullet
   in `commits.md`. Confirm:
   - Pre-commit hooks passed (the hook output is in the agent's
     pane).
   - The named tests in the acceptance row exist and pass under
     `cargo test --manifest-path rafaello/Cargo.toml -p
     rafaello-core` (or the fittings workspace command for c36).
   - For c11+, confirm fixture trees under
     `rafaello/crates/rafaello-core/tests/fixtures/` exist.

6. **Merge into rafaello-v0.1.**
   ```sh
   git checkout rafaello-v0.1
   git merge --ff-only agents/m1/c<NN>
   ```
   If the merge isn't a fast-forward, the agent branched from a
   stale base — rebase first, re-verify, then merge.

7. **Tear down.**
   ```sh
   tmux kill-session -t rafaello-m1-c<NN>-claude
   git worktree remove /home/luiz/lab-wt/m1-c<NN>
   git worktree prune
   ```
   Keep the agent branch as a ref for now; it gets pruned post-m1.

8. **Move on to the next commit.**

## Per-commit prompt template

Paste this verbatim, substituting `<NN>` for the commit number
and `<row>` for the entire commit row from `commits.md` (Subject
+ What + Why + Depends on + Acceptance):

```
You're implementing commit c<NN> for rafaello milestone m1-manifest.

Working environment:
- Worktree: /home/luiz/lab-wt/m1-c<NN>
- Branch: agents/m1/c<NN> (already created off rafaello-v0.1)
- Single-commit scope: this session lands ONE commit, then ends.

Read first:
- rafaello/plans/milestones/m1-manifest/scope.md (the m1 scope; ratified)
- rafaello/plans/milestones/m1-manifest/commits.md (your full milestone context)
- The relevant scope.md sections referenced by your commit row (M*, L*, T*, V*, C*, D*, G*, Tr*, K*, Sc*, Si*, W*, S*, E*)
- The current state of any source files this commit touches

Your commit row from commits.md:
<row>

Land exactly this commit. The acceptance criteria define "done":
- New tests live under rafaello/crates/rafaello-core/tests/ (or fittings/tests/ for c36)
- Tests land WITH the implementation in this same commit per
  ~/.claude/CLAUDE.md
- Pre-commit hooks (rustfmt + clippy + cargo test) MUST pass; never
  --no-verify. If a hook fails on pre-existing unrelated breakage,
  stop and ping the milestone driver
- Subject style: <type>(<scope>): <imperative>, matching the row
- One commit, one idea — do not bundle in unrelated cleanup
- The phase-boundary rule (commits.md, after c04): parse commits
  decode raw; grammar checks live in V1 (c10) and return
  ValidationError. Don't add grammar enforcement to parse-time
  unless the row says so.

When the commit has landed cleanly, end with the exact phrase on
its own line: COMMIT LANDED
```

The `COMMIT LANDED` sentinel lets you `tmux-wait` for completion
(after the standard sleep), then verify via `git log -1`.

## After c37 lands

1. **Retrospective.** Spawn claude+pi in tmux for
   `retrospective.md`:
   - Claude reviews the m1 diff against `scope.md` and
     `commits.md`, writes a draft retrospective answering:
     - Did anything in m1's implementation invalidate
       `overview.md`, `decisions.md`, or any stream RFC?
     - What slipped scope or got cut during implementation?
     - Coverage: every named test in scope's matrix landed?
   - Pi reviews adversarially.
   - Owner ratifies on convergence.
2. **Apply retrospective updates** to `overview.md`,
   `decisions.md`, and **stream F manifest RFC body** (the
   §15.1 normative-delta items 1–4 + the security RFC's
   `requires_confirmation` → `always_confirm` rename + helper /
   external-attach drift), and the **private-state path-key
   clarification** in overview §5.5 / decisions row 16 /
   glossary, as commits on `rafaello-v0.1`.
3. **Ping the owner.** m1 is done; m2 scoping starts when the
   owner says go.

`rafaello-v0.1` does NOT merge to `main` at m1 end — it
accumulates through m2–m6 and merges at v1 completion
(`decisions.md` row 33).

## Group-level checkpoints

Useful waypoints for owner sanity checks (no automatic gate,
just points where it's natural to pause and verify):

- **After c10**: parse + V1 grammar all green. Worked-example
  parsing won't be possible yet (no canonical_bytes), but the
  full parse + grammar + cross-ref surface compiles.
- **After c14 (and the m1a/m1b checkpoint at c18)**: parsers +
  lock + canonical id + topic-id + digest + sink inference all
  landed. **Driver re-evaluates m1a/m1b split here per the
  commits.md checkpoint**; default is to continue.
- **After c27**: V3 multi-plugin orchestration + every lock-side
  mirror green.
- **After c34**: compiler core complete; `cargo test -p
  rafaello-core` runs every named integration test in the
  scope matrix.
- **After c35**: broker ACL ready for m2's broker.
- **After c36**: fittings W cutover done; `cargo test
  --manifest-path fittings/Cargo.toml --workspace` green.
- **After c37**: m1 demo bar green; `manual-validation.md`
  written.

If something feels wrong at any checkpoint, the milestone
driver should pause and surface — don't barrel through.

## When things go wrong

Cross-references to the gotchas in `plans/README.md`:

- Per-commit agent doesn't land the commit / hangs / claims
  COMMIT LANDED but `git log` says otherwise → see "Verify
  commits actually landed".
- pi prints reviews but doesn't save → see "Pi sometimes prints
  reviews to chat without saving the file".
- `git commit` from orchestrator complains about
  `.pre-commit-config.yaml` not found → see
  `PREK_ALLOW_NO_CONFIG=1` gotcha.
- `tmux-wait` returns immediately → see "tmux-wait race after
  tmux send-keys".
- `nix develop` invocations (manual-validation, c37) fail with
  a "current directory" assertion → use `--impure` (m0
  retrospective §4.6).

When in genuine doubt, save the state (don't kill running tmux
sessions; commit any in-flight work to the agent branch) and
ping the owner.

## Quick command reference

Prep before starting any commit:
```sh
cd /home/luiz/lab
git checkout rafaello-v0.1
git pull --ff-only          # if applicable
```

Spawn per-commit agent (paste-buffer pattern preserves
multi-line prompts):
```sh
git worktree add /home/luiz/lab-wt/m1-c<NN> \
    -b agents/m1/c<NN> rafaello-v0.1

tmux new-session -d -s rafaello-m1-c<NN>-claude \
    -x 220 -y 50 -c /home/luiz/lab-wt/m1-c<NN>
tmux send-keys -t rafaello-m1-c<NN>-claude \
    "claude --dangerously-skip-permissions" Enter
sleep 2
tmux-wait rafaello-m1-c<NN>-claude "esc to interrupt" 60

cat <<'PROMPT' | tmux load-buffer -b m1-c<NN> - && \
    tmux paste-buffer -b m1-c<NN> -t rafaello-m1-c<NN>-claude && \
    tmux send-keys -t rafaello-m1-c<NN>-claude Enter
<paste the per-commit prompt template here, with substitutions>
PROMPT
sleep 15
tmux-wait rafaello-m1-c<NN>-claude "esc to interrupt" 3600
```

Verify + merge:
```sh
cd /home/luiz/lab-wt/m1-c<NN>
git log -1                   # confirm the commit exists
cd /home/luiz/lab
git checkout rafaello-v0.1
git merge --ff-only agents/m1/c<NN>
```

Cleanup:
```sh
tmux kill-session -t rafaello-m1-c<NN>-claude
git worktree remove /home/luiz/lab-wt/m1-c<NN>
git worktree prune
```
