# m2-broker-spawn commits.md — pi review round 4

> Review target: `rafaello/plans/milestones/m2-broker-spawn/commits.md`
> round-4 draft at commit `3c03137`, reviewed against ratified
> `scope.md` and `commits-pi-review-{1,2,3}.md`.
>
> Verdict: **ready to ratify**. The round-3 blockers and polish
> items are closed. I found no executable/structural blockers. The
> only remaining notes are count-wording polish that should not block
> Phase 3 per-commit agent work.

## Round-3 closure check

- **B1 — c20 fixture fd setup requires nonblocking before Tokio wrapping:**
  closed. c20 now converts `OwnedFd` to `std::os::unix::net::UnixStream`,
  calls `std.set_nonblocking(true)?`, then wraps with
  `tokio::net::UnixStream::from_std(std)?`.
- **B2 — c20 readiness before service registration:** closed. c20 now says
  `respond_peer_call` installs the fittings service first, then calls
  `core.fixture.ready`, preventing the harness from racing into
  `MethodNotFound`.
- **N1 — c30 four/five test count:** closed. c30 now says five tests and
  explicitly owns all five.
- **N2 — unknown-mode fixture filename drift:** closed. c20 refers to the
  c03 filename `tests/fixture_binary_unknown_mode_exits_64.rs`.
- **N3 — c25 pseudo-code `.drop()` syntax:** closed. c25 now uses
  `drop(managed.registered.take())` and `drop(managed.proxy.take())`.

## Blocking findings

None.

## Non-blocking polish

### N1. c16 says “Six new tests” but lists five

**Where:** c16 acceptance.

The listed files are the complete expected set for SP4 steps 4–7:
reserved env in `set`, reserved env in `pass`, invalid proxy allow-hosts,
entry not executable, and provider refusal. The count should say **Five**.
This is wording-only because all intended tests are explicitly named.

### N2. c20 says “Two new tests” while one is an existing regression

**Where:** c20 acceptance.

`tests/fixture_binary_unknown_mode_exits_64.rs` already lands in c03 and is
correctly called out as “already exists from c03”. c20 adds one new direct
fixture test and re-runs/verifies the existing unknown-mode regression. This is
wording-only because the filename and expected behaviour are clear.

## Verdict

**Ratifiable as-is:** 0 blocking + 2 non-blocking polish. The trajectory is
`7+many → 8+5 → 2+3 → 0+2`, with no remaining per-commit greenness trap or
scope-coverage gap found.
