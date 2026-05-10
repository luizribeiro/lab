# pi review 14 — m3 TUI sessions scope

Reviewed: `rafaello/plans/milestones/m3-tui-sessions/scope.md` at commit `91b07fa` (3077 lines).

## Verdict

Needs another revision.

The plan is close, but round 14 introduced or retained unsafe shutdown contradictions around `FrontendHandle::Drop`, dead reaper outcomes, and post-`SIGKILL` bounded waits. Those are lifecycle-critical enough to block implementation until clarified. The remaining issues are smaller but still actionable before writing `commits.md`.

## Blockers

### B1. `Drop` semantics still contradict round-14 shutdown rules

- `scope.md:948-952` says `Drop` SIGKILLs whenever `child_pid.is_some()`.
- `scope.md:1026-1034` says `Drop` first reads `reaper_outcome` and skips SIGKILL for any `Some(...)` outcome.
- `scope.md:959-978` explicitly says only `Exited(_)` proves the child is gone; `WaitFailed(_)` and `ReaperPanicked` mean the child may still be alive and must be cleaned up.

This contradiction reintroduces the exact round-13 regression round 14 claims to fix: treating `WaitFailed` / `ReaperPanicked` as proof-of-exit.

**Required fix:** make Drop-time skip-kill `Exited(_)-only`. If the cached outcome is `WaitFailed(_)`, `ReaperPanicked`, or `None`, Drop should still perform best-effort SIGKILL for a remaining `child_pid`.

### B2. `rfl chat` can hang before cleanup on `WaitFailed` / `ReaperPanicked`

- `scope.md:2010-2015` drains the stderr forwarder before teardown and says the await is bounded because the child stderr fd closes when the reaper has observed child exit.
- `scope.md:2065-2070` says `WaitFailed` / `ReaperPanicked` mean the child may still be alive, and shutdown must issue SIGTERM + SIGKILL.

For non-`Exited` outcomes, the child may still hold stderr open. Awaiting the forwarder before `frontend_handle.shutdown()` can therefore block forever and prevent the cleanup that would close stderr.

**Required fix:** split step-10 teardown by outcome. For `Exited(_)`, draining stderr before shutdown is fine. For `WaitFailed` / `ReaperPanicked`, call `shutdown()` first, then drain stderr, preferably with a bounded await/log-on-timeout policy.

## High

### H1. `shutdown_kill_grace` is defined but not actually used

- `scope.md:562-568` defines `shutdown_kill_grace` as the additional grace after SIGKILL.
- `scope.md:1934-1937` says readiness-error cleanup is bounded by `shutdown_grace + shutdown_kill_grace`.
- `scope.md:988-1006` uses `shutdown_grace` for the SIGTERM wait, then says after SIGKILL to use “another grace/sleep per step 4”, which reuses `shutdown_grace` rather than `shutdown_kill_grace`.

This makes the configured bound ambiguous and likely wrong.

**Recommended fix:** explicitly use `self.config.shutdown_grace` after SIGTERM and `self.config.shutdown_kill_grace` after SIGKILL, in both live-watch and dead-watch branches.

### H2. `SIGCONT` is not a safe liveness probe

- `scope.md:980-984` says to verify liveness by `nix::sys::signal::kill(Pid, Signal::SIGCONT)` returning `ESRCH`, describing it as a probe that “does not interrupt the process”.

`SIGCONT` can resume a stopped process, which is observable and can alter shutdown behavior.

**Recommended fix:** use a no-signal existence probe (`kill(pid, 0)` / nix equivalent). Treat `ESRCH` as gone; treat `Ok(())` and `EPERM` as alive/possibly alive.

### H3. Dead-watch PID probe is underspecified in the actual shutdown algorithm

- `scope.md:980-984` introduces an ESRCH probe for the `WaitFailed` / `ReaperPanicked` dead-watch path.
- `scope.md:996-1007` then describes the escalation check solely in terms of re-reading the cached `reaper_outcome`.

In a dead-watch path, the cached outcome will remain `WaitFailed` / `ReaperPanicked`; it will never become `Exited(_)`. The plan does not say how the probe result affects whether SIGKILL is sent or whether final non-exit confirmation is logged.

**Recommended fix:** spell out the dead-watch branch separately:

1. after SIGTERM grace, probe pid;
2. if probe says gone, skip SIGKILL;
3. otherwise send SIGKILL;
4. wait `shutdown_kill_grace`;
5. probe again;
6. warn if still alive/unknown.

## Medium

### M1. Critical round-14 branches are intentionally untested

- `scope.md:2043-2053` leaves `WaitFailed` / `ReaperPanicked` intentionally untested.

Those branches are no longer just defensive enum matches: round 14 changed them to drive the dead-watch shutdown path, including process signaling and PID probes. That path is now lifecycle-critical and easy to get wrong.

**Suggested fix:** add a small test seam for injecting `ReaperOutcome::WaitFailed` / `ReaperPanicked`, or add unit-level shutdown tests around a constructed handle/dead-watch harness. At minimum, test that these outcomes do not skip cleanup and do not block on stderr forwarding before shutdown.

### M2. Lock-holder PID parse failure is unspecified

- `scope.md:1236-1238` and `scope.md:1391-1394` say that on `EWOULDBLOCK`, the loser reads the holder pid from the lockfile and returns `SessionError::Locked { holder_pid }`.
- `scope.md:1239-1240` writes the holder pid only after acquiring and truncating the lockfile.
- `scope.md:1346-1349` defines `SessionError::Locked { holder_pid: u32 }` with no unknown/corrupt representation.

A contender can observe an empty, stale, partially written, or corrupt lockfile, especially around truncate/write timing or after external tampering. The current error shape cannot represent that.

**Suggested fix:** define the behavior explicitly, e.g. `Locked { holder_pid: Option<u32> }` or `LockedUnknownHolder`, and add a negative test for empty/corrupt lockfile contents under an active flock.
