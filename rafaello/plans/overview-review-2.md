# Pi review 2 — rafaello v1 architecture overview

Reviewed `rafaello/plans/overview.md` after Claude's revisions addressing `rafaello/plans/overview-review-1.md`.

Primary scope: `overview.md`. I also checked the changed stream RFCs and `decisions.md` where the overview now relies on cross-document deltas.

## Overall verdict

**Much improved. Not ready for final ratification yet.**

Claude addressed the round-1 overview issues well: the major architectural intent is now much clearer, especially around frontend process model, provider identity, bundled default provider, and manifest deltas. However, I found one new transport-level blocker and several consistency issues that should be cleaned up before final ratification.

## Round-1 findings status

### 1. Fittings notification primitive — mostly addressed, but exposed a deeper transport gap

Round-1 finding: `overview.md` assumed core could fan out notifications using a fittings primitive that Stream B had deferred.

Round-2 status: **partially resolved.**

`overview.md` now explicitly promotes connection-scoped `ServerHandle::notify` to a v1 fittings requirement:

- `overview.md` §4.1 describes why `ServiceContext::notify` is insufficient and sketches `Server::handle() -> ServerHandle` plus `ServerHandle::notify(...)`.
- `overview.md` §15.6 makes this a required Stream B follow-up.
- `decisions.md` decision 22 records the promotion.

This resolves arbitrary core→plugin/frontend **notification** fan-out. But it also reveals a deeper request/response problem covered in the new blocking finding below: `ServerHandle::notify` is not sufficient for core↔plugin RPC such as `renderer.render`.

### 2. Namespace id semantics — addressed

Round-1 finding: `provider.<id>.*`, `plugin.<id>.*`, and `frontend.<id>.*` overloaded `<id>`.

Round-2 status: **resolved in the overview.**

`overview.md` §4.3 now distinguishes:

- `provider.<provider-id>.*` — human-readable provider id from lock bindings;
- `plugin.<topic-id>.*` — hashed canonical plugin id;
- `frontend.<attach-id>.*` — session-scoped frontend attach id.

This is the correct split. Minor polish remains in later shorthand references; see Minor polish below.

### 3. Frontend process model — addressed

Round-1 finding: the overview contradicted itself on whether the TUI was in-process or a spawned frontend, and whether frontends were sandboxed like plugins.

Round-2 status: **resolved in the overview.**

`overview.md` §3 now states frontends are always separate processes from core, with the default TUI as a separate `rfl-tui` subprocess. It also explicitly states frontends are trusted UI principals, not lockin-sandboxed plugins. `overview.md` §12 now aligns interactive vs daemon mode with that model.

### 4. Default provider model — addressed

Round-1 finding: the overview said providers ship as plugins but also said the default provider was built into core.

Round-2 status: **resolved in the overview.**

`overview.md` §8.1 now models the default provider as a bundled-but-real subprocess plugin named `rfl-litellm`, with normal lock/grant/sandbox/bus semantics. That preserves the “providers ship as plugins” goal.

A stale Security RFC comment still says absent provider means “use the built-in default provider”; see Cross-document drift below.

### 5. Manifest contract gaps — improved, but not fully clean

Round-1 finding: the manifest contract was too vague for ratification and had filename/topic/eager-failure inconsistencies.

Round-2 status: **substantially improved, with remaining issues.**

`overview.md` §15.1 now gives a concrete normative delta for Stream F, including:

- `[provides]` block;
- `tools`;
- `provider`;
- `helpers`;
- `[provides.tool.<name>]` metadata;
- `sinks`;
- `grant_match`;
- `requires_confirmation`;
- top-level `helper_for`;
- install-time validation rules;
- explicit removal of the speculative `load.eager_failure` manifest knob.

The security RFC filename was harmonized to `rafaello.toml`, and Stream F's `fs.changed:**/*.rs` examples were partly rewritten.

Remaining issues:

- `requires_confirmation` is internally contradictory; see High-priority finding 2.
- Stream F still has stale topic-grammar and provider examples; see Medium-priority finding 5.

### 6. Bus schema ownership — addressed in overview, but some stream drift remains

Round-1 finding: ownership of bus event schemas was contradictory between overview and Security RFC.

Round-2 status: **resolved in the overview.**

`overview.md` §15.2 now pins the split:

- Stream A owns broker/event payload schemas and semantics.
- Stream B owns JSON-RPC envelopes and wire framing.

Security RFC §9 item 3 was also patched to match. This is now a good canonical split.

### 7. Sink-confirmation rule — addressed in overview, but stream summary remains stale

Round-1 finding: overview and Security RFC differed on whether sink confirmation is taint-independent.

Round-2 status: **resolved in overview; stale Security RFC summary remains.**

`overview.md` §6.2 now clearly states the canonical v1 rule:

> Any tool_request whose target tool declares one or more sink classes is held unless a matching `user_grants` entry covers the invocation. The rule is independent of whether args carry taint.

This is the right conservative rule. `decisions.md` decision 9 also records it.

However, Security RFC §10 still says confirmation applies when args carry non-user taint and the target declares a sink. The overview calls that out as stale, but it should be patched before final ratification or explicitly listed as a ratification follow-up.

### 8. Streaming topic spelling — addressed in overview; Stream E drift remains

Round-1 finding: the overview still used `session.entry.*` in some places despite choosing `core.session.entry.*`.

Round-2 status: **resolved in overview.**

`overview.md` now consistently uses `core.session.entry.*` for streaming entry topics.

Stream E still uses unprefixed `session.entry.*`; see Cross-document drift below.

## Blocking finding

### 1. `ServerHandle::notify` is insufficient for core↔plugin request/response over one fd

The revised `overview.md` fixes notification fan-out by promoting `ServerHandle::notify`, but §4.1 now says something stronger:

- each plugin runs a fittings server on its bus fd;
- core runs a fittings server on the other end;
- both ends therefore use both fittings client and server APIs.

Later, `overview.md` §11.1 says `renderer.render` is a request/response method on the renderer plugin's own fittings server, with cancellation propagation if a frontend disconnects.

The problem: Stream B v1 still explicitly defers server-originated requests / bidirectional request handling. `ServerHandle::notify` gives core a way to push **notifications** outside a handler; it does not give core a way to issue request/response calls to plugin servers over the same connection while also serving plugin→core calls.

Affected cases include at least:

- core calling plugin RPC methods such as `renderer.render`;
- core dispatching tool calls if they are represented as request/response rather than one-way bus events;
- plugin simultaneously using `Client::notify("bus.publish", ...)`;
- sharing one transport safely between client and server loops in both directions.

References:

- `overview.md` §4.1, especially the “both ends therefore use both fittings client and server APIs” paragraph.
- `overview.md` §11.1, `renderer.render` as request/response on the renderer plugin's fittings server.
- `streams/b-fittings/rfc-fittings-notifications.md` “Server-originated requests: v1 cut” and “Future work”, which defer server→client requests and associated pending-response machinery.

**Required fix:** choose one of these designs before final ratification:

1. Promote a full duplex fittings peer / combined client+server transport to v1, with request/response in both directions over one fd; or
2. remove direct core→plugin request/response from the bus fd design and express those interactions as bus events with explicit reply topics / correlations; or
3. allocate separate fds/connections for opposite request directions and specify the lifecycle/authentication model.

As written, the overview assumes a v1 transport capability that neither Stream B nor the new `ServerHandle::notify` requirement actually specifies.

## High-priority findings

### 2. `requires_confirmation` is internally contradictory

`overview.md` uses `requires_confirmation` in two incompatible ways.

In §5.3 it is listed as:

- `requires_confirmation` (advisory)
- purpose: “Hint, not enforcement”

In §15.1, the normative manifest delta says:

- `requires_confirmation = false` is “advisory hint; not enforced” in the example comment;
- but the install-time validation rules then say `requires_confirmation = true` causes core to also gate the tool through confirmation even when `sinks = []`.

Those cannot both be true.

**Required fix:** pick one semantics:

- advisory only, no core behavior; or
- enforced opt-in confirmation, probably renamed to something clearer like `always_confirm` or documented as “enforced UX gate, not a security sink”.

This matters because it changes whether read-only tools can force human review and whether manifest authors can influence runtime confirmation behavior.

### 3. `decisions.md` marks decisions as ratified before ratification

`decisions.md` defines `ratified` as “agreed by owner + claude + pi after debate”. But all 25 rows are already marked `ratified` or `locked`, while `overview.md` still says Pi has not reviewed and the owner ratifies on convergence.

References:

- `decisions.md` status definitions.
- `decisions.md` rows 1–25.
- `overview.md` status banner.

This creates process ambiguity: are these decisions already ratified, or are they proposed decisions awaiting owner sign-off?

**Required fix:** either:

- mark them as proposed/pending until owner ratification; or
- update the process/status banner if ratification has actually happened.

## Medium-priority findings

### 4. Upstream RFC drift remains for promoted contracts

The overview is the source of truth, so some stream drift is acceptable during convergence. But several stale lines are high-impact enough that they should be patched immediately or listed as explicit ratification follow-ups.

#### 4.1 Stream B still lists connection-scoped `ServerHandle` as future work

`overview.md` §15.6 promotes `ServerHandle::notify` to v1. But `streams/b-fittings/rfc-fittings-notifications.md` still lists connection-scoped `ServerHandle` under “Future work”.

This is exactly the kind of drift that can cause implementation divergence.

#### 4.2 Security RFC §10 still has the stale sink-confirmation rule

`overview.md` §6.2 and `decisions.md` decision 9 make sink confirmation taint-independent. But `streams/a-security/rfc-security-model.md` §10 still says:

> any tool_request whose args carry non-user taint and whose target tool declares any sink class...

That should be patched to match the overview.

#### 4.3 Stream E still uses unprefixed `session.entry.*`

`overview.md` is canonical on `core.session.entry.*`, but `streams/e-renderer/rfc-renderer-model.md` still uses `session.entry.appended`, `session.entry.patched`, and `session.entry.finalized` throughout the architecture and examples.

Patch or explicitly list as a required follow-up.

#### 4.4 Security RFC still refers to a built-in default provider

The overview now says the default provider is bundled `rfl-litellm`, not built into core. But `streams/a-security/rfc-security-model.md` §3.2 still says absent or null `provider_active` means “use the built-in default provider”.

Patch to “bundled default provider” or specify the absence behavior under the new model.

### 5. Stream F still has stale topic/provider examples

Claude partially patched Stream F topic examples, but there are still stale or misleading bits.

#### 5.1 Stream F §4 still points to the wrong grammar

`streams/f-manifest/rfc-manifest-schema.md` §4 now uses `core.fs.changed`, which is better, but still says:

> Topic strings follow Stream B's fittings ACL grammar (`namespace.event[:filter]`).

That contradicts the overview / Stream A ownership split. Topic grammar and ACL are Stream A broker semantics, not Stream B fittings semantics. The `[:filter]` notation is also stale.

#### 5.2 Stream F provider example still publishes invalid provider topics

Stream F §9.3 provider example still has:

```toml
[bus]
publishes  = ["provider.tokens", "provider.cost"]
subscribes = []
```

Under the canonical namespace, provider publishes should be under `provider.<provider-id>.*`, e.g. `provider.anthropic.tokens`, or the example should be rewritten to use the new `[provides] provider = ...` model once Stream F is patched.

### 6. Minor overview polish

These are not blockers, but should be cleaned up before finalizing the document.

#### 6.1 Duplicate subscribe-authority sentence

`overview.md` §4.3 repeats:

> Subscribe authority is per-pattern, granted by the lock and checked on every delivery.

Remove the duplicate.

#### 6.2 Shorthand id placeholders after §4.3 are still imprecise

After §4.3 carefully distinguishes provider-id, topic-id, and attach-id, later sections still use shorthand like:

- `provider.<id>.tool_request`;
- `plugin.<targeted-id>.tool_request`;
- `plugin.<id>.tool_result`;
- `provider.<parent>.spawn_helper`.

This is understandable shorthand, but the whole point of the round-1 fix was avoiding id-type ambiguity. Prefer `provider.<provider-id>.*`, `plugin.<topic-id>.*`, and `frontend.<attach-id>.*` in load-bearing examples.

#### 6.3 Formatting in §11.1 is awkward

The bullet beginning:

> `core.session.entry.*` events use the connection-scoped server notification handle (§4.1, fittings' notification path)

is hard to read because the line break splits the parenthetical. Not semantically wrong, but worth polishing.

## Final summary

Round-1 issues are mostly resolved in `overview.md`, and the architecture is converging. The main remaining problem is that the transport story is not fully implementable yet: `ServerHandle::notify` fixes notification fan-out but does not solve core↔plugin request/response over one fd. That needs a concrete v1 design before final ratification.

After that, clean up the `requires_confirmation` contradiction, align `decisions.md` status with the actual ratification process, and patch or explicitly track the remaining stream-RFC drift.