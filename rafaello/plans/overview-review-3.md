# Pi review 3 — rafaello v1 architecture overview sign-off review

Reviewed `rafaello/plans/overview.md` after Claude addressed `rafaello/plans/overview-review-2.md`, including the structural bidirectional core↔plugin transport issue.

## Overall verdict

**Not quite signed off yet — needs a small cleanup pass.**

The major structural issue from round 2 is now resolved: bidirectional core↔plugin transport is explicitly modeled as `PeerHandle` in `overview.md` §4.1 / §15.6 and Stream B.

However, `overview.md` still contains two direct contradictions that block final sign-off.

## Bidirectional-transport assessment

The round-2 blocker was that `ServerHandle::notify` solved core→plugin notifications but not core↔plugin request/response over one fd.

Round-3 status: **resolved structurally.**

`overview.md` now defines every bus fd as a bidirectional fittings peer connection. Each side runs both client and server roles over the same fd, and the v1 transport primitive is a symmetric `PeerHandle` with:

- `notify(method, params)` for outbound notifications;
- `call(method, params).await` for outbound request/response;
- `closed()` for lifecycle observation.

This is specified in:

- `overview.md` §4.1;
- `overview.md` §15.6;
- `decisions.md` decision 22;
- `streams/b-fittings/rfc-fittings-notifications.md` “Bidirectional `PeerHandle` (v1)”.

This addresses the implementation gap for:

- core calling `renderer.render` on renderer plugins;
- core pushing `core.session.*` fan-out notifications;
- frontend capability negotiation paths;
- helper-spawn handshakes;
- simultaneous calls in both directions on one transport.

The remaining problems are not with the `PeerHandle` design itself, but with stale overview text that still describes the old “notifications only” v1 cut.

## Blocking issues

### 1. `overview.md` §1.2 still says bidirectional requests are deferred

`overview.md` §1.2 still says:

> No server→client JSON-RPC requests in fittings. v1 ships notifications only; bidirectional requests are deferred.

This contradicts:

- `overview.md` §4.1, which defines bidirectional fittings peer connections;
- `overview.md` §15.6, which promotes `PeerHandle` with `notify` and `call` to v1;
- `overview.md` §16 “In v1”, which includes bidirectional `PeerHandle`;
- `decisions.md` decision 22;
- Stream B’s new bidirectional `PeerHandle` section.

**Required fix:** replace the §1.2 bullet with a non-goal that matches the new design, for example:

> No higher-level sampling/elicitation protocol in v1; the fittings transport supports bidirectional `PeerHandle::call`, but specific human-in-the-loop protocol methods are deferred.

### 2. `overview.md` §16 still lists server-originated fittings requests as v2

`overview.md` §16 “In v1” includes:

> fittings v1 with `ServiceContext`, bidirectional `PeerHandle` (notify + call in both directions, §15.6), cancellation, two-channel server loop, predefined error preservation.

But the same section’s “Deferred to v2” table still includes:

> Server-originated fittings requests | Notifications cover v1 needs; deferred per Stream B

These cannot both be true.

**Required fix:** remove that deferred row, or rewrite it to defer only higher-level protocols layered on `peer.call`, such as sampling/elicitation method definitions.

## Round-2 findings status

### 1. Bidirectional core↔plugin request/response transport

Round-2 verdict: `ServerHandle::notify` was insufficient for core↔plugin request/response.

Round-3 status: **resolved structurally.**

The overview and Stream B now define bidirectional `PeerHandle` with both `notify` and `call` over one fd.

### 2. `requires_confirmation` contradiction

Round-2 verdict: `requires_confirmation` was internally contradictory because it was both advisory and enforced.

Round-3 status: **resolved in overview.**

The field has been renamed to `always_confirm`, with enforced UX-gate semantics:

- `overview.md` §5.3 lists `always_confirm` as per-tool and enforced;
- `overview.md` §15.1 explains the rename and drops the advisory-hint interpretation.

One stale Stream A mention remains; see Non-blocking cleanup.

### 3. `decisions.md` premature ratification

Round-2 verdict: `decisions.md` marked decisions as `ratified` before owner sign-off.

Round-3 status: **resolved.**

`decisions.md` now defines `proposed` and marks the rows as `proposed`, pending owner ratification.

### 4. Upstream RFC drift for promoted contracts

Round-2 verdict: several stream RFCs still had stale text for promoted contracts.

Round-3 status: **mostly resolved.**

Resolved:

- Stream B now includes bidirectional `PeerHandle` as v1.
- Security RFC §10 now matches the taint-independent sink-confirmation rule.
- Stream E now uses `core.session.entry.*`.
- Security RFC §3.2 now models no active provider / bundled `rfl-litellm` correctly.

Remaining cleanup:

- Security RFC §9 item 2 still references `requires_confirmation` advisory.
- Stream B retains superseded sections that are marked historical, but one earlier section still describes server-originated requests as unsupported before the later superseding section.

### 5. Stream F stale topic/provider examples

Round-2 verdict: Stream F still had stale grammar wording and invalid provider topics.

Round-3 status: **resolved.**

Stream F §4 now points to Stream A’s broker grammar and removes the stale `namespace.event[:filter]` contract. Stream F §9.3 now uses `[provides] provider = "anthropic"` and publishes under `provider.anthropic.*`.

### 6. Minor overview polish

Round-2 verdict: duplicate subscribe sentence, imprecise id placeholders, and awkward §11.1 formatting.

Round-3 status: **resolved.**

The duplicate sentence is gone, load-bearing examples now use `provider-id` / `topic-id` / `attach-id` placeholders, and §11.1 reads cleanly.

## Non-blocking cleanup list

These are not architecture blockers, but should be cleaned up before or shortly after ratification.

1. **`overview.md` §15.2 stale patch note.** It still says Security RFC §9 item 3 is stale and must be patched, but that patch has already landed.
2. **`overview.md` §15.3 stale Stream E note.** It still says Stream E uses unprefixed `session.entry.*`, but Stream E has already been patched to `core.session.entry.*`.
3. **Security RFC §9 item 2 stale field name.** It still lists `requires_confirmation` advisory, while overview now uses enforced `always_confirm`.
4. **Stream B historical text can mislead skimmers.** The “Client-side: server-originated requests arriving when not supported” section still describes the old unsupported behavior before the later `PeerHandle` section supersedes it. Add an inline pointer there or move the superseded material lower.

## Final summary

The architecture is very close. I would sign off after the two `overview.md` contradictions are patched:

1. §1.2 must stop saying bidirectional fittings requests are deferred.
2. §16 must stop listing server-originated fittings requests as v2.

The remaining items are editorial drift cleanup, not architecture blockers.