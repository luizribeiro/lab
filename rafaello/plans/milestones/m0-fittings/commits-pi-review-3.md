# Pi review 3 — m0 fittings commits sign-off review

Review target: `rafaello/plans/milestones/m0-fittings/commits.md` after
`4ea5835 docs(rafaello-m0): apply pi-review-2 fixes to commits.md`.

Verdict: **sign-off pending two small acceptance-traceability fixes**.
Claude addressed the round-2 blockers: c09 no longer requires pre-existing
`peer.call`, `service_context_notify.rs` is explicit, c10 uses a raw
harness until K2 lands, c19 completes the public client notification path,
`FittingsError::Panic` and dispatcher panic mapping are explicit, error
preservation has an end-to-end test, and full manual validation moved to a
final c32.

The remaining issues are not architectural rewrites, but they matter
because `commits.md` says every named scope-matrix test must land.

## Remaining must-fix before owner ratification

### 1. `id_namespace_isolation.rs` is no longer explicitly planned

Scope's demo matrix requires `id_namespace_isolation.rs`: server- and
client-initiated `peer.call`s use disjoint namespaces, with concurrent
calls proving no collisions (`scope.md:285`). The revised c11 only unit
tests the standalone allocator (`commits.md:266-280`), and c14's
`peerhandle_bidirectional.rs` acceptance covers bidirectional calls but
not the 100-concurrent/no-collision isolation test.

**Fix:** add `tests/id_namespace_isolation.rs` to c14 acceptance (or c30
if the driver wants it with the heavier transport regressions). Prefer
c14: once both sides' `peer.call` paths exist, run 100 concurrent calls in
each direction on one connection and assert all responses correlate.

### 2. `bounded_notify_drop.rs` lost the scope-required post-flood `peer.call`

Scope's row for `bounded_notify_drop.rs` says the handler floods
notifications, drops are counted, and a subsequent `peer.call` succeeds
(`scope.md:282`). c09 correctly avoids using `peer.call` before it exists,
but now its acceptance only requires ordinary request/response traffic
post-flood (`commits.md:236-242`). The later peer-call tests do not
explicitly tie back to the post-flood bounded-notify scenario.

**Fix:** keep c09's ordinary request/response check, but explicitly extend
`tests/bounded_notify_drop.rs` after `peer.call` exists (c14 is the natural
place) with the scope-required post-flood `peer.call` assertion.

## Editorial cleanup (non-blocking, do with the same patch)

- Header status still says “Pi review 2 pending” (`commits.md:3-5`).
  Update it for round-3/owner ratification.
- Scope names `error_marker_round_trip.rs` (`scope.md:278`); c04 covers
  the marker behavior under `wire_inbound_error_round_trip.rs`. Either
  use the scope file name or add a one-line mapping so the named-test
  acceptance is unambiguous.
- c02 acceptance says the Panic variant participates in “data and
  message” construction tests (`commits.md:82-85`), but `Panic` itself
  has only `message`; its marker `data` is produced by c04. Reword to
  avoid implying `FittingsError::Panic` has a data field.

After the two acceptance-traceability fixes above, I would sign off on
`commits.md` for owner ratification.
