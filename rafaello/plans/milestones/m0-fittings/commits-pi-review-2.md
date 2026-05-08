# Pi review 2 — m0 fittings commits

Review target: `rafaello/plans/milestones/m0-fittings/commits.md` after
Claude's rewrite in `991581f docs(rafaello-m0): rewrite commits.md per pi-review-1`.

Verdict: **much improved, but not ratifiable yet**. The big round-1
sequencing problem is mostly fixed: c08 is now an explicit green cutover,
future-dependent acceptances were moved, and cancellation config now
precedes dropped-future cancellation. The remaining blockers are smaller
and localized, but a few acceptance criteria are still impossible at the
commit where they appear, and one error-marker API change is not explicitly
planned.

## Blocking findings

### 1. c09 acceptance still uses `peer.call` before it exists

c09 is the bounded-notification-channel commit, but its acceptance says
that after notification flooding a subsequent `peer.call` succeeds
(`commits.md:202-204`). `PeerHandle::call` is not implemented until c12
(server side) and c14 (client side), so c09 cannot satisfy this test.

**Fix:** make c09 assert the S4 contract directly: notification drops
increment the counter and subsequent ordinary request/response traffic
still succeeds. The later bidirectional `peer.call` regression is already
covered by c14/c30.

### 2. c09/c10 do not actually land `service_context_notify.rs`

The ratified demo bar includes `service_context_notify.rs`: a handler
emits five notifications mid-request and the client receives all five
before the response. The rewritten plan wires basic `ctx.notify` in c08
and bounded notify in c09, but no commit acceptance names or clearly
covers that ordering test. c09 only covers flood/drop behavior; c10 only
covers outside-handler peer notify.

**Fix:** add `tests/service_context_notify.rs` to c09 (or c08 if the
basic channel is already meant to guarantee ordering). It should assert
the non-drop path: five handler-emitted notifications arrive before the
response.

### 3. c10 requires client-side notification observation before K2 exists

c10 acceptance says startup tasks on each side notify the peer and “both
sides observe the inbound notification” (`commits.md:215-217`). But the
public client-side inbound notification handler is not introduced until
c19. Unless c10 uses a raw test transport instead of `Client`, the client
cannot observe server-originated notifications through the public API yet.

**Fix:** either:

- scope c10's acceptance to `Server::peer().notify` plus a raw harness,
  and move the `Client::peer().notify`/client-observation public test to
  c19; or
- move the sync notification handler earlier so c10's “both sides” test
  is implementable.

### 4. `FittingsError::Panic` / panic marker is not explicitly planned

c04 says Transport/Panic markers round-trip and that server panic produces
`data.fittingsKind = "panic"` (`commits.md:90-101`), but no commit adds a
`FittingsError::Panic` variant to `fittings-core`, and the server panic
path is not called out in the c08 dispatcher cutover. Current code maps
worker panics to plain `Internal`; without an explicit core/server change,
`error_marker_round_trip.rs` cannot prove the RFC marker contract.

**Fix:** add `Panic { message }` explicitly, probably in c02 alongside
the other `FittingsError` shape changes, and make c08 (or a dedicated
server commit) map handler panics through `FittingsError::Panic` so the
wire marker is exercised. Also fix c04's stale function name
(`to_fittings_error` should match the actual inbound mapping API) and the
`S5/marker` citation.

### 5. Manual validation is scheduled too early / incompletely

c29 says it captures JS-SDK output in `manual-validation.md`
(`commits.md:476-486`), but Group 6 transport/spawn verification lands
afterward, and scope manual validation includes the full negative suite,
`cargo build -p fittings`, and Linux/macOS evidence. A `manual-validation.md`
written at c29 cannot be the final milestone validation record.

**Fix:** add a final docs/validation commit after c31, or change c29 to
record only JS interop output and add an explicit later step for the full
`manual-validation.md` required by milestone acceptance.

## High-priority should-fix

### 6. Error-preservation acceptance should include the end-to-end behavior

c03/c04 are wire mapping tests (`commits.md:77-101`). The scope demo bar's
`error_preservation_round_trip.rs` is stronger: a server returns each
predefined error with custom `message`/`data`, and the client receives
both byte-equal. Add a clear integration-test acceptance (or a mapping
note explaining where that named behavior lands), otherwise the plan may
satisfy wire units while missing the client/server round trip.

### 7. c17 has a stale cross-reference

c17 says token-firing logic is c20 (`commits.md:307-310`), but c20 is now
`id_null_explicit_request`; token firing is c21. This is editorial but
confusing because c20 was moved in response to review.

### 8. Keep constructor compatibility explicit in c02

c02 says constructors “accept and store” the new data fields
(`commits.md:66-69`). In Rust, changing existing helpers from
`invalid_params(message)` to require a data argument would cause needless
workspace churn. The errors RFC says constructor helpers stay for source
compatibility. Wording should say existing one-arg helpers set `data:
None`, with additional data-bearing constructors or direct variant
construction available.

## Answers to Claude's open items

- **c08 size:** the consolidated cutover is the right trade-off. A
  temporary two-trait shim would be more confusing than one large,
  explicitly-green API cutover.
- **c20 placement:** move it earlier if convenient — ideally right after
  c08/c09, before bidirectional call work. It is dispatcher/in-flight
  foundation, not really Group 3 peer-handle work. Not a blocker if c21
  remains the first hard dependency, but earlier is safer.
- **c11 dependency:** if it is a standalone allocator module with unit
  tests, c01 is enough. If it is embedded inside `PeerHandle` internals,
  depend on c07/c10. The plan should state which.
- **c25 placement:** semantically it belongs after Group 3 (after c16 and
  c19), not Group 4; it asserts the peer delivery/closed contract more
  than cancellation routing. Placement is not blocking as long as the
  dependencies are complete.

## Minor cleanup

- c06 says suppression arrives in c23 (`commits.md:129-131`), but the
  suppression commit is c22.
- `commits.md:572` calls peer-gone-during-notify “c30”; it is c25 in the
  current plan.
