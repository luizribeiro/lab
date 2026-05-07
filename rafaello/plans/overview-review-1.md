# Pi review 1 — rafaello v1 architecture overview

Reviewed `rafaello/plans/overview.md` on branch `agents/overview/pi` against the stream RFCs in `rafaello/plans/streams/`.

No files were edited during the review itself.

## Overall verdict

**Needs revision before ratification.**

The synthesis is strong, especially the broker/transport layering and the taint/sink model, but there are several contract-level inconsistencies that should be resolved before `overview.md` becomes the ratified v1 source of truth.

## High-priority / blocking findings

### 1. Fittings v1 does not actually provide the notification primitive the overview assumes

`overview.md` assumes core can fan out bus events and stream entries as fittings notifications, and that plugins can publish onto the bus with:

```text
ctx.notify("bus.publish", { topic, payload, in_reply_to, taint? })
```

References:

- `overview.md` §4.1, especially the concrete publish/fan-out path.
- `overview.md` §11.1, especially entry streaming over notifications.
- `streams/b-fittings/rfc-fittings-notifications.md` §2, where `ServiceContext::notify` is per-request.
- `streams/b-fittings/rfc-fittings-notifications.md` Future work, where connection-scoped `ServerHandle` for eventbus-style push is deferred.

The mismatch: Stream B’s v1 `ServiceContext::notify` is a handler context that notifies the same peer during a request. It is not a general connection-scoped outbound notification handle suitable for core broker fan-out or session event streaming outside a currently executing request.

**Required fix:** either make a connection-scoped notification handle (`ServerHandle::notify` or equivalent) a v1 fittings requirement, or change the bus transport design so it does not depend on a primitive Stream B defers.

### 2. Namespace `<id>` semantics are wrong or ambiguous in the overview

`overview.md` §4.3 says `<id>` is the plugin topic-id form for all namespace rows:

- `provider.<id>.*`
- `plugin.<id>.*`
- `frontend.<id>.*`

But Stream A uses three different id meanings:

- `provider.<provider-id>.*`, e.g. `provider.camel.tool_request`.
- `plugin.<topic-id>.*`, where topic-id is the hashed canonical plugin id.
- `frontend.<attach-id>.*`, e.g. `frontend.tui.confirm_answer`.

References:

- `overview.md` §4.3.
- `streams/a-security/rfc-security-model.md` §3.2 provider bindings.
- `streams/a-security/rfc-security-model.md` §5.2 namespace table.

This is implementation-significant because provider ids are human-selected lock bindings, plugin ids are collision-checked hash renderings, and frontend ids are attach principals.

**Required fix:** split these explicitly in the overview; only `plugin.<id>` should use the hashed topic-id form.

### 3. Frontend process model contradicts itself

`overview.md` describes the default TUI in two incompatible ways:

- §10 says the local-spawned TUI is spawned by core like a plugin, with a socketpair and `RFL_BUS_FD`.
- §12 says interactive mode has the TUI “spawned in-process”.

The process-model section also says every non-core process is sandboxed by lockin and reaches the rest of the system only through fds core hands it. That is not true for external attached frontends, and it is unclear whether it is intended to be true for local frontends.

References:

- `overview.md` §3 process model.
- `overview.md` §10 frontend model.
- `overview.md` §12 daemon mode.

**Required fix:** choose one TUI model and state frontend trust/sandboxing separately from plugin sandboxing. If frontends are trusted UI principals rather than hostile plugins, the process-model language should say so.

### 4. Built-in default provider conflicts with the “providers ship as plugins” goal

`overview.md` goals say providers ship as plugins and plugins are the unit of capability. Later, §8 says the default v1 provider is built into core and talks to the LiteLLM endpoint.

References:

- `overview.md` §1.1 goals 2–3.
- `overview.md` §8 provider model.
- `plans/README.md` Tooling notes for the LiteLLM endpoint and default model.

This leaves unclear how the built-in provider participates in:

- provider identity (`provider.<id>.*`);
- lock/grant semantics;
- environment/API-key handling;
- network authority;
- taint and tool-routing invariants.

A built-in provider may be acceptable, but it needs to be modelled explicitly so it cannot accidentally bypass the plugin/provider rules the rest of the architecture depends on.

**Required fix:** make the default provider a bundled pseudo-plugin with explicit lock/grant/bus semantics, or revise the minimal-core/plugin-unit claims.

### 5. Manifest contract gaps are still too concrete for ratification

The overview correctly identifies several Stream F gaps, but v1 depends on them. This makes the contract insufficiently settled for m1 implementation.

References:

- `overview.md` §5.3 required manifest fields.
- `overview.md` §15.1 cross-stream manifest gaps.
- `streams/f-manifest/rfc-manifest-schema.md` §2–§11.
- `streams/a-security/rfc-security-model.md` §3.1 and §9.

Specific issues:

1. Missing Stream F fields are load-bearing for v1:
   - `provides.tools`;
   - provider role / `provides.provider`;
   - per-tool `sinks`;
   - per-tool `grant_match`;
   - helper relationship / `helper_for`;
   - `requires_confirmation` advisory hint.
2. Manifest filename mismatch remains:
   - overview and Stream F say `rafaello.toml`;
   - Stream A still says `plugin.toml`.
3. `overview.md` introduces `load.eager_failure = "open"`, but Stream F has no such schema field and leaves eager failure as an open question.
4. Stream F still uses invalid topic examples such as `fs.changed:**/*.rs`, even though the overview says the canonical topic grammar forbids `:` and `/`.

**Required fix:** either patch Streams A/F before ratification or make `overview.md`’s §15 a precise normative delta list that implementers can follow without guessing.

## Medium-priority findings

### 6. Bus event schema ownership is contradictory

`overview.md` says Stream A owns the bus-level schema and Stream B owns only JSON-RPC framing. Stream A’s “what we still owe” section says Stream B must commit to the bus event schemas.

References:

- `overview.md` §15.2.
- `streams/a-security/rfc-security-model.md` §9 item 3.

This is not just editorial: bus event envelopes include security-significant fields (`taint`, `in_reply_to`, `request_id`), while fittings owns transport-level JSON-RPC envelopes. Duplicate ownership invites divergent schemas.

**Recommended fix:** align ownership explicitly. The likely split is: Stream A owns broker/event envelopes and semantics; Stream B owns JSON-RPC framing and transport behavior.

### 7. Sink confirmation condition is inconsistent across docs

The overview says any sink call without a matching `user_grants` entry requires confirmation. Stream A’s final summary says sink confirmation applies when args carry non-user taint and the target tool declares a sink.

References:

- `overview.md` §6.2.
- `overview.md` §6.4.
- `streams/a-security/rfc-security-model.md` §7.2.3.
- `streams/a-security/rfc-security-model.md` §10 summary.

This affects both UX and the security claim:

- If every sink call confirms unless covered by `user_grants`, the system is conservative but potentially noisy.
- If only tainted sink calls confirm, then untainted or user-only-tainted sink calls may bypass confirmation unless separately handled.

**Recommended fix:** make one canonical rule explicit in both the overview and Stream A. In particular, clarify whether an untainted sink call and a user-only-tainted sink call require confirmation absent `user_grants`.

### 8. Overview still uses old unprefixed streaming topic names in its own fittings-composition section

The overview correctly declares that canonical streaming topics are under `core.session.entry.*`, but later uses the old unprefixed `session.entry.*` spelling in §11.1.

References:

- `overview.md` §11, canonical spelling.
- `overview.md` §11.1, stale spelling.
- `streams/e-renderer/rfc-renderer-model.md` §7, source of the stale spelling.

**Recommended fix:** replace the remaining overview references to `session.entry.*` with `core.session.entry.*`. Stream E can remain documented as drift if that is the chosen workflow, but the overview should be internally canonical.

## Summary

The architecture direction is sound, but the above issues should be addressed before ratification. The most important fixes are to settle the fittings notification primitive, clarify namespace id semantics, choose a frontend process model, normalize the built-in provider against the plugin/provider security model, and make the manifest contract implementable.