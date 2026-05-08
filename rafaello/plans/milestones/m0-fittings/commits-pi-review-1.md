# Pi review 1 — m0 fittings commits

Review target: `rafaello/plans/milestones/m0-fittings/commits.md` as drafted by Claude for Phase 2 step 5.

Verdict: **do not ratify as-is**. The draft covers the ratified scope at a high level, but several commits cannot land green in order, and a few dependencies/acceptance criteria refer to APIs that are introduced only later. Fix those sequencing problems before owner ratification; otherwise the Phase 3 per-commit agents will dead-end.

## Blocking findings

### 1. The plan explicitly allows non-green intermediate commits

`commits.md:19-21` says every commit is gated by pre-commit hooks/tests, but c08's acceptance says downstream workspace crates “don't yet” compile until c09–c14 (`commits.md:135-138`). That violates the per-commit green bar in `plans/README.md` and the conventions in this file.

This is not just a wording issue: changing `Service::call` in `fittings-core` also forces `Middleware` to change immediately, because current middleware calls `next.call(req)` and lives in the same crate. Splitting C2 and C4 across c08/c12 while claiming `fittings-core` compiles is internally inconsistent.

**Fix:** make every commit independently green. Either:

- combine the breaking trait, middleware, macro, server, client/example compile fixups into one larger “API cutover” commit; or
- land temporary compatibility shims/default context constructors so each intermediate commit still builds the workspace, then remove shims in a later green commit.

### 2. Multiple acceptances depend on future commits

Several commits require behavior that their dependencies do not provide:

- c16 is only an id-namespace strategy/doc/helper commit, but its acceptance requires 100 concurrent `peer.call`s in both directions (`commits.md:235-237`). That cannot pass before c17/c18/c19.
- c17 acceptance says the client responds “with a registered `Service` from c19” (`commits.md:247-249`), but c19 is later.
- c23 tests both directions of in-handler `ctx.peer().call`, but depends only on c11+c17 (`commits.md:317-321`); client-side peer/service support from c18/c19 is also needed.
- c25 acceptance says cancellation fires a token (`commits.md:347-349`), but c25 does not depend on c24, the token-plumbing commit.
- c27's token-fired integration test depends on the configured cancellation reader from c25/c26, not just c06+c24 (`commits.md:369-374`).
- c31 exercises `ctx.notify` and pending calls but depends only on c20 (`commits.md:416-421`); it also needs c15.
- c36 bidirectional transport tests need client inbound service registration (c19), not just c20 (`commits.md:483-486`).
- c37 says `SubprocessConnector` can `peer.call`/`peer.notify` with only c10 as a dependency (`commits.md:494-498`), but c10 is notify-only; this needs the full call/client-service work.

**Fix:** move the relevant acceptance tests to the first commit where all required APIs exist, or reorder the commits so the dependencies are real. For c17 specifically, either test against a raw test-harness responder, or move `Client::with_service` earlier and make c17 depend on it.

### 3. Dropped-future cancellation is ordered before cancellation configuration exists

c21 emits “the configured cancellation method” (`$/cancelRequest` default) when a `peer.call` future is dropped (`commits.md:287-298`), but cancellation configuration is introduced later in c25. This makes the implementation contract underspecified at c21.

**Fix:** introduce a shared cancellation config/default before c21, or move dropped-future cancellation after c25. The test should also cover the configured MCP override eventually, not only the LSP default.

### 4. MCP migration commits are under-dependent and acceptance is premature

c32 depends only on c15, but `mcp-server` is a macro consumer and participates in the new `ServiceContext` signature cutover. The title of c14 says only hello examples, while the body says all examples; `mcp-server` must be included in the green cutover or explicitly deferred with a compiling adapter.

c33's acceptance requires cancellation interop and response suppression (`commits.md:446-448`), but c33 depends only on c25+c32. That behavior also needs the cancellation reader/token/suppression work (c24/c26/c27) and the handler migration in c34.

**Fix:** make `mcp-server` compile at the trait/macro cutover point, and move end-to-end MCP cancellation acceptance after all cancellation implementation and handler migration have landed.

## High-priority corrections

### 5. c06 describes `Cancelled` backwards

c06 says `Cancelled` is an “explicit non-suppression-trigger return value for handlers” (`commits.md:107-109`). The ratified scope says handler-returned `Err(FittingsError::Cancelled)` is one of the two independent suppression triggers. c27 later states the correct rule.

**Fix:** change c06 wording to “no wire mapping; handler-returned suppression trigger once the server implements S6.”

### 6. ServiceContext cancellation is duplicated between c07 and c24

c07 exposes `cancelled()`/`is_cancelled()` and tests token semantics (`commits.md:118-126`), while c24 later says it adds the per-request cancellation token and wires those same methods (`commits.md:327-336`).

**Fix:** either put the token implementation in c07 and make c24 only connect real peer cancellation into existing tokens, or keep c07 as a shape-only/inert context and move token semantics tests to c24.

### 7. c01/c02 are stale or imprecise relative to the current code shape

The current wire `RequestEnvelope.id` is already `Option<JsonRpcId>`; the lossy part today is server/core conversion into `fittings_core::message::Request { id: String }`. c01's wording (“migrate `Request.id: JsonRpcId`”) does not match the current code.

Similarly, c02 talks about a “wire error enum”, but the typed error variants are `fittings_core::error::FittingsError`; `fittings-wire` only maps to/from envelopes.

**Fix:** retarget these commits to the actual crates/types that must change, or explicitly say they are tightening existing wire behavior plus adding regression tests.

### 8. Error-code acceptance misses one valid band from scope wording

c05 tests positive, server-band, and below-reserved negative codes (`commits.md:95-99`). Scope C5 says any negative outside `-32768..=-32000` is valid; that also includes the above-reserved negative band such as `-31999`/`-1` unless the RFC is amended.

**Fix:** add a valid above-reserved-negative case, and consider testing code `0` as invalid in addition to a predefined/reserved code.

## Answers to Claude's open items

- **Commit count/split:** 37 is fine; the problem is not count, it is greenability and dependency correctness. Some commits should merge (trait/middleware/macro cutover), while some tests should move later.
- **c30 placement:** move `id_null_explicit_request` runtime semantics earlier. It should land before cancellation routing, because cancellation/in-flight maps need correct `JsonRpcId::Null` behavior. A good place is after the server dispatcher has the first real in-flight/request tracking, not in late Group 4.
- **c16 id namespace:** string prefixes `s_<n>`/`c_<n>` are sane and easy to debug. Use one allocator abstraction, document that generated ids cannot collide with the opposite side's generated namespace, and treat peer-supplied duplicate ids separately at the in-flight map.
- **Test consolidation:** do not optimize for fewer files yet. Move tests to the right commits first; consolidate only if implementation agents find setup duplication painful.
- **m0a/m0b cut:** c19 is not a good cut while c20–c23 are required to complete Group 3 acceptance. A cleaner provisional cut is after the API/notify cutover is fully green (current Group 2, after corrected c15), with all bidirectional call/client-service/closed/drop-cancel work in m0b. If owner wants an m0a that includes usable bidirectional calls, cut after corrected Group 3 instead.

## Minor cleanup

- `commits.md:31-32` says m0b is c20–c41, but the draft ends at c37.
- c10 says call comes in c14 and closed in c16 (`commits.md:157-159`); current draft has call in c17 and closed in c20.
- c11 says the `ctx.peer().call` test lights up in c14 (`commits.md:174-177`); it is c23/c17+.
- c26 acceptance says “second request's cancellation” while the saturated sleeping handler is presumably the request that should observe the token (`commits.md:357-360`). Clarify the target request id.
