# m2-broker-spawn driver notes

Operational notes from the milestone driver during Phase 3.
Append-only; new entries on top.

## c23 publish_one hang diagnosis 2026-05-10

**Root cause.** The bus fan-out / observer path was not the
problem. The publisher fixture reached the publish path, but
then tried to use the flush ack as
`client.call("core.fixture.after_publish", Value::Null)`. The
fittings wire decoder rejects any present `params` that is not
an object or array, so that request is an `InvalidRequest`
(`field \`params\` must be an object or array when present`).
That made the publisher panic after sending `bus.publish`, which
sent agents chasing broker/subscription races and, when they
used unbounded waits or the outer cargo timeout killed the test,
left the long-lived observer/sydbox children orphaned. The fix is
to send object params (`json!({})`) for the flush ack (or omit
params in a future fittings API), and keep integration-test waits
bounded.

**Verification.** After changing the ack params to `{}`,
`timeout 30s cargo test -p rafaello-core --features test-fixture
--test fixture_publish_one_emits_event -- --nocapture` exits via
the test harness rather than the timeout. A broader
`timeout 45s cargo test -p rafaello-core --features test-fixture
--tests -- --nocapture` also exits via the harness.

## c22 restructure 2026-05-09

**What happened.** c22 round-1 prompt asked for three
supervisor-driven integration tests (`fixture_publish_one_emits_event`,
`fixture_call_core_then_exit_completes`,
`fixture_dump_env_returns_allow_listed_keys`) alongside the
fixture-mode dispatch additions. The agent hung for >1h trying
to make `fixture_dump_env_returns_allow_listed_keys` work. The
hang root cause: those tests call methods like
`core.fixture.dump_env` and `core.fixture.observed` that the
production `BusPublishService` (c19) routes only `bus.publish`
through; the production supervisor returns `MethodNotFound` for
everything else. The required test wiring is the
`with_extra_service` constructor + `ExtraServiceFactory` shape
that the c23 harness ships. c22 attempted to thread that ahead
of c23 and got stuck on the missing extra-service plumbing.

**Resolution.** Killed c22 round-1 (worktree + branch + 21
runaway fixture processes). Restructured commits.md:
- c22 now ships only the fixture-mode dispatch additions +
  a build-only compile assertion. ~10x smaller scope.
- The three deferred integration tests move to c23 alongside
  the harness they actually depend on.

**Lesson for future per-commit prompts.** Tests requiring
`with_extra_service` extra-service plumbing must land in or
after c23 (the harness commit), never before. The c22 round-1
mistake was assuming "the c21 supervisor surface is enough" —
it isn't, because c21 only exposes the production
`BusPublishService` route.

**Cleanup performed.**
- `tmux kill-session -t rafaello-m2-c22-claude`
- `pkill` of all `/m2-c22/` rfl-bus-fixture + sydbox + nix-develop processes (21 dangling)
- One c20-vintage fixture leak also killed (had been alive for 1h 14m — orphaned by c20 worktree removal; root cause: fixture's `respond_peer_call` mode sleeps until SIGTERM and the SandboxedChild Drop didn't fire when the test crashed).

## Per-commit pattern (live)

Per commit:
```
1. git worktree add /home/luiz/lab-wt/m2-c<NN> -b agents/m2/c<NN> rafaello-v0.1
2. ln -s /nix/store/.../pre-commit-config.json /home/luiz/lab-wt/m2-c<NN>/.pre-commit-config.yaml
   (worktrees outside /home/luiz/lab don't get the symlink — c17 hung on this until added)
3. tmux new-session -d -s rafaello-m2-c<NN>-claude -x 220 -y 50 -c /home/luiz/lab-wt/m2-c<NN>
4. claude --dangerously-skip-permissions in the session
5. paste-buffer the per-commit prompt with the FULL row text + every acceptance bullet inlined verbatim (per m1 §4.2 — never cite by row number)
6. wait via Monitor on "esc to interrupt" disappearing from the pane
7. verify commit landed via git log -1 in the worktree AND verify /home/luiz/lab git status is clean (the c02/c03 silent-merge-failure gotcha)
8. git checkout rafaello-v0.1 && git merge --ff-only agents/m2/c<NN>
9. tmux kill-session + git worktree remove + git branch -D
```

## Driver-side gotchas hit so far

- **Cargo.lock dirty in /home/luiz/lab silently aborts ff-merges.** Fix: stash Cargo.lock before merging if dirty. Symptom: `git worktree add` shows the OLD rafaello-v0.1 tip even after the agent's commit "merged".
- **`.pre-commit-config.yaml` symlink missing in worktrees outside /home/luiz/lab.** Fix: symlink it at worktree creation time. Without the symlink, `git commit` errors with "config file not found" and agents waste cycles trying to work around it.
- **Agents may leak fixture child processes if the test crashes.** The fixture's `respond_peer_call` mode sleeps until SIGTERM. m2's supervisor Drop (c26) is supposed to handle this for in-test instances, but not when the test panics before the SpawnedChild's Drop runs. Watch for runaway fixture procs across worktrees.

## Branch model

Already documented in `commits.md` "Conventions" section.
