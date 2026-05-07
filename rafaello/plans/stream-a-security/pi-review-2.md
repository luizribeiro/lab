# pi review 2 — revised rafaello v1 security RFCs

Reviewer: pi on branch `agents/stream-a/pi`  
Scope: Claude's revised stream-a RFCs after `pi-review-1`:

- `rfc-security-model.md`
- `rfc-camel-on-v1.md`

## Final verdict

The revision is **much improved** and addresses most round-1 blockers
substantively. The core security RFC is now close to implementable: the
bus transport is lockin-compatible, provider authority is no longer mixed
with `core.*`, topic grammar is canonical, lock bindings now snapshot
runtime authority, carve-outs are framed in terms lockin can express, and
v1 now has mandatory taint plus core-mediated sink confirmation.

I would call this **close, but not execution-ready**. Remaining issues are
mostly consistency/specification gaps, with a few still security-relevant.
The most important remaining fix is to remove stale CaMeL prompt text that
contradicts the revised security RFC, then make an owner decision on the
helper-plugin primitive.

## Strengths in the revision

- **Round-1 bus blocker resolved.** The UDS + token design was replaced
  with an inherited socketpair fd (`RFL_BUS_FD`), which is compatible with
  lockin's deny-by-default network model.

- **Provider/core namespace conflict resolved.** Providers publish on
  `provider.<id>.*`; core validates and re-emits canonical `core.*` events.
  This is the right shape for keeping `core.*` authoritative.

- **Canonical topic grammar added.** The revised security RFC now has one
  topic grammar and removes the earlier `:`/`.` ambiguity.

- **Lock bindings added.** Tool names, provider role, renderer kinds, and
  sink metadata are snapshotted into the lock, which fixes the earlier
  contradiction between runtime routing and "manifest not consulted at
  spawn time."

- **Filesystem carve-outs are now lockin-realistic.** The draft no longer
  assumes deny-subpath precedence that lockin does not provide; it uses
  refusal/decomposition instead.

- **Cross-tool exfiltration is treated as a v1 concern.** Mandatory taint
  on both `tool_result` and `tool_request`, plus core-enforced sink
  confirmation, is a substantial improvement over the first draft.

- **CaMeL dependency claims are more honest.** The CaMeL RFC no longer
  claims "no v1 gap" unconditionally; it lists dependencies row-by-row and
  marks helper-plugin spawn as an explicit gap.

## Findings

### 1. CaMeL RFC still has stale protocol instructions

**Verdict:** blocker before handing the prompt to an implementation agent.

`rfc-camel-on-v1.md` still contains text from the old model that
contradicts the revised security RFC:

- §2, "What the v1 primitives give you", item 1 says CaMeL publishes
  `core.session.tool_request`. Under the revised model it should publish
  `provider.camel.tool_request`, with core re-emitting
  `core.session.tool_request` after validation.
- §2, item 3 says taint is `taint: [string, ...]`. The security RFC §7.2
  changed taint to structured objects, e.g. `{ "source": "web", "detail":
  "example.com" }`.
- The Q-LLM test still references connecting to `/tmp/bus.sock`, but the
  revised bus model has no bus path; plugins receive an inherited fd.

These stale instructions will misdirect the v2 implementation agent. The
CaMeL prompt should be mechanically updated to match the security RFC's
provider namespace, structured taint schema, and fd-based bus model.

### 2. `bindings.helper_for` remains an unresolved v1 dependency

**Verdict:** blocker for the clean CaMeL architecture; acceptable only if
v2 explicitly chooses the degraded in-process fallback.

The CaMeL RFC now correctly marks helper-plugin spawning as row 10 and a
remaining gap. However, that means clean CaMeL is not yet actually
"buildable on v1".

Open details that need to be specified if `helper_for` is accepted:

- If `camel-qllm` has no `RFL_BUS_FD`, how does CaMeL communicate with it?
- Is the communication channel stdio, fittings subprocess transport, a
  core-mediated RPC, or another inherited fd?
- Who owns lifecycle, cancellation, logs, and failure propagation?
- Does `camel-qllm` get installed/updated independently or as a helper
  artifact of `camel`?
- CaMeL's manifest still requests network access to both P-LLM and Q-LLM
  endpoints; if Q-LLM is supposed to be isolated in `camel-qllm`, CaMeL
  itself should not also need direct Q-LLM egress in the clean design.

#### Helper-plugin opinion

If clean CaMeL is a v2 goal, I recommend **accepting `bindings.helper_for`
as a v1 primitive now**, but only with a small, explicit contract:

1. Helper plugins are normal installed plugins with their own digest,
   manifest snapshot, lock entry, and lockin policy.
2. Helpers are not visible as tools/providers/renderers unless they also
   declare those roles independently.
3. Core, not the parent plugin, spawns helpers.
4. Helpers may be spawned without a bus fd.
5. Parent ↔ helper communication must be one named primitive, preferably
   a core-owned stdio/fittings channel, not an ad-hoc side channel.
6. The parent plugin's lock binding authorizes exactly which helper ids it
   can request.

If the project owner does not want that surface in v1, then the CaMeL RFC
should remove the clean-helper path from the implementation prompt and
make the in-process Q-LLM design the official v2 fallback.

### 3. Frontend namespace/auth is used but not in the bus namespace table

**Verdict:** major security/specification gap.

Security RFC §5.2 lists three top-level namespaces:

- `core.*`
- `provider.*`
- `plugin.<id>.*`

But §5.6 introduces `frontend.<id>.confirm_answer`, and frontends can
approve security-sensitive actions. That makes frontends first-class
security principals, not an incidental detail.

The RFC should add frontends to the namespace and trust model:

- Add `frontend.<id>.*` to the publish-authority table.
- Define how frontends authenticate.
- Clarify how external frontends attach if they are not spawned by core
  with inherited socketpairs.
- State whether frontends are trusted as user-authorized UI principals or
  treated as untrusted clients whose answers are only accepted after a
  stronger local-user check.

Without this, the confirmation protocol is underspecified at exactly the
point where user consent enters the security model.

### 4. Tool-result routing is underspecified

**Verdict:** major completeness issue.

The revised RFC clearly defines request routing:

```text
provider.<id>.tool_request
  -> core validates/synthesizes taint
  -> core.session.tool_request
  -> plugin.<id>.tool_request
```

But the corresponding result path is not specified with the same clarity.
Plugins cannot publish `core.*`, while providers subscribe to
`core.session.tool_result`. Therefore the RFC should explicitly define:

```text
plugin.<id>.tool_result
  -> core validates/canonicalizes/synthesizes taint
  -> core.session.tool_result
  -> provider/frontends/subscribers
```

This matters because taint, `in_reply_to`, request/result correlation, and
provider visibility all depend on result routing being core-mediated.

### 5. “User-only taint” is not sufficient authorization

**Verdict:** major security issue.

Security RFC §7.2.3/§7.2.4 says sink calls with only `{source: "user"}`
taint may proceed without confirmation. The rationale is that data from
the user message authorizes the sink the user named.

That conflates two concepts:

- **User data provenance:** the bytes came from a user message.
- **User authorization:** the user asked for this sink/action.

Those are not equivalent. Example: the user pastes a secret, API key,
email address, or private note into the prompt. A prompt-injected or
misbehaving LLM could send that user-originated data to a sink and bypass
confirmation because the taint is user-only.

The RFC should distinguish user data from user-granted sink capability.
Possible models:

- Treat `{source: "user"}` as provenance only; still require confirmation
  for irreversible/network sinks unless the user explicitly granted that
  sink in a structured way.
- Add a separate `user_grants` / `allowed_sinks` field derived from the
  initial user request, with conservative extraction and confirmation when
  ambiguous.
- Keep v1 simple: user-only taint does not bypass sink confirmation except
  for a narrow allowlist of obviously harmless sinks.

As written, the user-only bypass is too broad.

### 6. Taint inheritance can be bypassed by omitting `in_reply_to`

**Verdict:** major security issue for hostile plugins.

Security RFC §7.2.2 says a plugin includes `in_reply_to` and the broker
enforces published taint as a superset of the referenced request ids. It
also says events without `in_reply_to` are treated as having no inherited
taint.

For hostile plugins, optional `in_reply_to` creates a taint-stripping path:
just omit it.

The fix is to make `in_reply_to` mandatory for event classes that are
semantically replies or transformations:

- `plugin.<id>.tool_result` must reference exactly the tool request it
  answers.
- RPC replies must reference the RPC call.
- Core should reject result/reply events missing the required correlation.
- Optional `in_reply_to` is acceptable only for unrelated telemetry such
  as progress updates.

This keeps the broker/core from relying on plugin honesty for taint
inheritance.

### 7. Topic-form plugin id rendering can collide

**Verdict:** major correctness/security footgun.

Security RFC §5.1 renders plugin ids into topic segments by replacing
`:`, `/`, and `@` with `_`. That mapping is not collision-safe. For
example:

```text
github:acme/grep@1.4.2
github_acme_grep_1_4_2
```

can render to the same topic segment.

Because publish authority is namespace-based, topic id collisions are not
just cosmetic. Use a reversible encoding, a hash suffix, or explicit
collision rejection at lock time. A simple safe option is:

```text
plugin.<base32url(sha256(canonical_plugin_id))[0..16]>.*
```

with the canonical id retained in payloads/logs for readability.

### 8. Private state write grants must be excluded from “workspace write”

**Verdict:** major implementability/UX issue.

Every plugin automatically gets recursive read/write access to:

```text
${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/
```

But §7.1 defines `has_workspace_write = write_dirs non-empty`. If the
private state grant counts, then every plugin has workspace write. A
networked provider or web tool with private state could trip the trifecta
rule unexpectedly. If it does not count, the RFC should say so explicitly.

Recommended wording: `has_workspace_write` excludes the automatic private
state grant and only counts user/project write authority outside the
plugin's private state subtree.

Related: sink defaults should treat filesystem-write tools as sinks, not
only network-capable tools. A tool with `write_dirs` but no network still
may be an irreversible sink for tainted data.

### 9. “Lethal trifecta broken at the plugin boundary” is now stale wording

**Verdict:** minor consistency issue, but worth fixing because it affects
security claims.

The revised model no longer relies only on plugin-boundary trifecta
breaking. It relies on bus-level taint propagation and core-mediated sink
confirmation for cross-tool flows.

The goal statement should be updated from "broken at the plugin boundary"
to something like:

> The lethal trifecta is reduced per plugin and cross-tool exfiltration is
> gated at the bus by mandatory taint + sink confirmation.

### 10. “Single envelope field” CaMeL dependency wording is stale

**Verdict:** minor consistency issue.

Security RFC §1 still says CaMeL is buildable as a v2 plugin modulo the
single envelope field documented in §8. That was true of the first draft,
but the revised CaMeL RFC now lists many v1 contracts and one open gap.

Update the goal/non-goal wording to match the new dependency table.

### 11. RFC status still says first pass

**Verdict:** minor editorial issue.

`rfc-security-model.md` still says:

```text
Status: draft, stream-a, first pass for review.
```

It should say revised/round-2 draft or similar.

### 12. Pattern grammar should be separated from topic grammar

**Verdict:** minor spec clarity issue.

The topic grammar says:

```text
segment := [a-z0-9_-]+
```

but subscribe patterns use `*` and `**`, which are not valid topic
segments. This is fine conceptually, but the RFC should define a separate
`pattern` grammar so implementers do not accidentally treat wildcard
segments as valid concrete topics.

### 13. `provider_active` placement in the lock is unclear

**Verdict:** minor schema issue.

The RFC says switching providers records `bindings.provider_active =
"<plugin-id>"`, but provider active selection sounds global/session or
project-level, not a field inside one plugin's bindings. Clarify where it
lives in `rafaello.lock` and how it interacts with multiple installed
provider plugins.

### 14. DNS ownership warning is probably not implementable as stated

**Verdict:** minor implementability issue.

§6.7 says install warns when a plugin's `allow_hosts` includes any
non-wildcard hostname the user is not the publisher of. Reliably knowing
whether the user is the publisher/controller of a hostname is hard.

Prefer simpler wording: warn on all allowlisted hosts not already trusted
by policy, all wildcards, and any host that resolves to private/link-local
ranges during install/update checks. Keep ownership claims out unless
there is a concrete verification mechanism.

## Recommended next actions

1. **Patch stale CaMeL RFC protocol lines** to match the revised security
   RFC: provider namespace, structured taint, fd-based bus, and no
   `/tmp/bus.sock` references.

2. **Make a go/no-go owner decision on `bindings.helper_for` now.** If yes,
   specify helper spawn, communication, lifecycle, and lock schema. If no,
   make in-process Q-LLM the official CaMeL v2 path.

3. **Add frontend principals to the bus model.** Include namespace,
   authentication, attach flow, and trust assumptions for confirmation
   answers.

4. **Specify result routing explicitly.** Define plugin result topic → core
   validation/taint/correlation → canonical `core.session.tool_result`.

5. **Replace the user-only taint sink bypass.** Distinguish user data
   provenance from explicit user authorization for a sink/action.

6. **Make `in_reply_to` mandatory where taint inheritance matters.** At
   minimum, require it on tool results and RPC replies and reject missing
   correlations.

## Closing assessment

Round 2 is a large improvement and shows the RFCs are converging. I would
not block further design discussion on the broad architecture anymore; the
shape is good. But I would block implementation handoff until findings 1,
2, 3, 4, 5, and 6 are resolved or explicitly accepted as scoped residual
risk.
