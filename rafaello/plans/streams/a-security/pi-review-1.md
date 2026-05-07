# pi review 1 — rafaello v1 security RFCs

Reviewer: pi on branch `agents/stream-a/pi`  
Scope: Claude's draft stream-a RFCs:

- `rfc-security-model.md`
- `rfc-camel-on-v1.md`

## Summary verdict

The direction is strong: the draft has the right high-level shape —
manifest as request, lock as grant, lockin as runtime enforcer, with the
LLM treated as adversarial input. However, the RFCs are not yet
implementation-ready or security-authoritative. Several claims depend on
primitives that are not defined in v1, conflict with existing lockin
semantics, or are internally inconsistent across the two documents.

The biggest fixes needed before implementation are:

1. Define a bus transport model that works under lockin.
2. Resolve provider/plugin authority over `core.*` topics.
3. Make filesystem carve-outs implementable with current lockin, or ask
   lockin for a new primitive.
4. Add a stronger v1 story for LLM-mediated exfiltration across tools.
5. Canonicalise the topic grammar, tool/provider binding model, and
   confirmation protocol.

## Findings

### 1. Bus transport conflicts with lockin

`rfc-security-model.md` says plugins connect to a Unix-domain socket at
`${XDG_RUNTIME_DIR}/rafaello/<session-id>/bus.sock` and authenticate with
`RFL_BUS_TOKEN`.

That conflicts with lockin as currently documented:

- `sandbox.network.mode = "deny"` blocks arbitrary AF_UNIX outbound.
- `sandbox.network.mode = "proxy"` also restricts outbound to the proxy
  path/port and is not a general UDS allow mechanism.
- The RFC's env scrubbing rule strips `*_TOKEN`, which would strip
  `RFL_BUS_TOKEN` unless it is specially exempted.

This is a blocker because every plugin needs bus access, including
plugins with network denied.

Possible fixes:

- Pass a pre-connected bus fd/socketpair into the plugin instead of
  requiring a sandboxed UDS connect.
- Reserve `RFL_BUS_TOKEN` from credential env scrubbing and document it
  as core-injected authority, not inherited user secret.
- Add explicit UDS path allow support in lockin and include it in the
  compiled policy.

### 2. `core.*` namespace contradicts the CaMeL provider model

`rfc-security-model.md` defines `core.*` as published by the agent core
only, and says plugins may never publish it.

`rfc-camel-on-v1.md` requires the CaMeL provider plugin to publish:

- `core.session.tool_request`
- `core.session.assistant_message`

The CaMeL RFC adds a parenthetical provider exception, but that exception
is not present in the security RFC's namespace authority model.

This must be resolved before CaMeL can be authoritative. Options:

- Provider plugins publish provider-owned topics, and core validates and
  re-emits canonical `core.*` events.
- Define explicit provider-only exceptions in the bus ACL.
- Model CaMeL as middleware rather than as a provider that publishes
  `core.*`.

### 3. Filesystem carve-outs are not implementable with current lockin

The security RFC promises broad directory grants, such as
`${PROJECT_ROOT}`, while carving out subpaths like:

- `${PROJECT_ROOT}/rafaello.lock`
- `${PROJECT_ROOT}/.rafaello/**`
- credential directories under `${HOME}`
- hidden directories except specific plugin state

Current lockin `read_dirs` and `write_dirs` are recursive allowlists. The
docs do not describe deny-subpath precedence inside an allowed recursive
directory.

Therefore a grant like `write_dirs = ["${PROJECT_ROOT}"]` cannot safely
mean "write the project except `rafaello.lock` and `.rafaello/**`" using
current lockin primitives.

Possible fixes:

- Do not compile broad recursive grants when carve-outs are needed;
  decompose access into narrower concrete paths.
- Require plugin manifests to request specific directories instead of
  workspace root writes.
- Add deny-subpath support to lockin.
- Downgrade the carve-out claim to a UX/compiler restriction that is only
  enforceable when the grant can be represented precisely.

### 4. v1 still allows LLM-mediated exfiltration across tools

The RFC breaks the lethal trifecta per plugin, but the actual agent risk
is often cross-tool:

1. A read-only plugin reads a sensitive value and returns it to the LLM.
2. The LLM passes that value as an argument to a network-only plugin.
3. No single plugin has all three legs of the trifecta.

The draft adds taint to `tool_result`, but it does not make taint
mandatory on `tool_request` arguments or require sink tools to enforce it.
The RFC explicitly says v1 does no enforcement on taint values and relies
on CaMeL or tool opt-in.

That means v1 should not claim that the lethal trifecta is structurally
broken at the plugin boundary. It is only partially reduced unless v1
adds mandatory provenance/sink enforcement or mandatory user confirmation
for tainted egress.

Possible fixes:

- Carry taint/provenance through `tool_request` envelopes.
- Require core to block or confirm tainted data flowing to network/write
  sinks.
- Make `refuses_tainted_input` more than a manifest hint for v1 built-ins.
- Downgrade the v1 security claim and explicitly state that full
  LLM-mediated exfiltration prevention is v2/CaMeL.

### 5. Topic grammar is inconsistent

The security RFC uses multiple incompatible topic forms:

- Lock example: `session.tool_request:grep.*`
- Namespace section: `core.session.tool_request`
- Subscribe examples: `core.session.tool_result:grep.*`
- Plugin namespace: `<plugin-id>.*`

It also says globs are allowed within a single dot segment, but examples
use `:` as a secondary separator.

The broker ACL cannot be implemented safely without a single grammar.
The RFC needs:

- Allowed characters and escaping for plugin ids.
- Whether `:` is part of the topic grammar or a payload discriminator.
- Glob syntax and segment boundaries.
- Concrete matcher examples.
- Rules for provider/tool-specific routing without overloading topic
  strings ambiguously.

### 6. The CaMeL “no v1 gap” claim is too strong

`rfc-camel-on-v1.md` says there is no v1 gap that blocks CaMeL as a
plugin, provided the taint envelope ships.

That is too strong. The prompt also depends on:

- Provider plugin authority over tool requests.
- A bus model where the provider really is the sole LLM-to-tool path.
- Child execution with independent lockin policy.
- Private plugin state writes for audit logs.
- A confirmation request/reply protocol.
- Tool sink metadata for policy decisions.

Several of these are not defined in the security RFC or are deferred to
Stream F / Stream B. The RFC should say CaMeL is plausible on v1 only if
these contracts are included in v1.

### 7. “Q-LLM as child plugin with its own lockin policy” is not supported by current fittings spawn API

The CaMeL RFC instructs the implementation agent to spawn the Q-LLM as a
child plugin using the fittings spawn API with its own lockin policy.

Current fittings subprocess support appears to spawn a normal subprocess
for JSON-RPC serving. It does not by itself orchestrate lockin policy
compilation or sandboxed child-plugin authority. The RFC should not cite
fittings spawn as if it already provides that isolation.

Possible fixes:

- Have rafaello core own all plugin spawning, including CaMeL's Q-LLM
  helper.
- Add an explicit stream-B/F requirement for sandboxed subprocess
  spawning.
- Reword the CaMeL prompt to say the plugin uses rafaello/lockin process
  orchestration, not bare fittings spawn.

### 8. CaMeL says “no filesystem grants” but requires audit logs

The CaMeL manifest instructions say CaMeL needs no filesystem grants.
Later, the prompt requires refusals to be logged under:

```text
.rafaello-plugin-data/camel/audit/<session>.jsonl
```

That requires write access to the plugin private state directory. The
security RFC mentions a private plugin state dir, but the CaMeL RFC needs
to request that grant explicitly or define it as an automatic per-plugin
capability.

### 9. Confirmation flow introduces undeclared bus contracts

The CaMeL RFC introduces:

- `camel.confirm_request`
- `camel.confirm_reply`

and assumes a frontend can participate. This is effectively a protocol
contract, not just an implementation detail.

It conflicts with the current plugin namespace model because
`camel.confirm_reply` appears to be under the plugin-owned namespace, yet
it would be published by a user-facing frontend or core.

The RFC needs a minimal confirmation protocol:

- Topic names and who may publish them.
- Request/reply schema and correlation id.
- Timeout behavior, preferably fail-closed.
- Whether confirmations are core-mediated or frontend-mediated.
- How the requesting plugin learns the answer without allowing arbitrary
  plugins to spoof user consent.

### 10. Tool/provider bindings are referenced as lock authority but absent from the lock schema

The security RFC says a tool request is delivered only to the unique
plugin whose lock grants `provides.tools = ["grep"]`, and provider
selection is also treated as a locked authority decision.

However, the lock schema example only shows `read_dirs`, `write_dirs`,
`exec_paths`, `network`, `env_pass`, `subscribes`, and `publishes`. It
does not show locked tool/provider bindings.

This also conflicts with the statement that the manifest is not consulted
at spawn time. If runtime routing uses `provides.tools` or
`provides.provider`, those fields must be copied into and authorised by
the lock.

The lock schema should explicitly include granted provider/tool bindings,
conflict resolution, and update/reconfirmation behavior.

## Other important issues

- **`manifest not consulted at spawn time` needs careful wording.** The
  runtime compiler may avoid consulting the current manifest, but it still
  needs manifest-derived fields snapshotted into the lock: tool names,
  provider role, subscriptions, publications, renderer registrations, and
  possibly sink metadata.

- **`refuses_tainted_input` as a manifest hint is weak.** If this is only
  advisory, it should not appear as a v1 mitigation for prompt-injection
  exfiltration. If it is enforceable, the enforcement point and event
  schema need to be specified.

- **Taint superset enforcement is underspecified.** The broker cannot know
  which input data a plugin actually used merely because the plugin
  subscribed to prior events. The RFC should define whether taint is
  session-global, per-request, correlation-id-based, or explicit in tool
  request/result envelopes.

- **Trifecta graph analysis is underspecified.** `has_outbound` depends on
  another plugin subscribing and having network. The RFC should specify
  whether this is direct-only or transitive, how cycles are handled, and
  which graph is evaluated at grant/lock time.

- **Private-IP filtering conflicts with lockin's current documented trust
  model.** lockin currently documents that it does not filter address
  classes after DNS resolution. The RFC correctly calls this an extension,
  but the main mitigation language should stay conditional until lockin
  accepts the change.

- **Carve-out language is inconsistent.** The RFC calls credential and lock
  file carve-outs unconditional/non-overridable, then introduces
  `--allow-credential-paths`. Use one model: either truly non-overridable,
  or default-denied with an explicit dangerous override.

- **Project hidden directory semantics are hard to enforce recursively.**
  The default rule excluding hidden directories under `${PROJECT_ROOT}` has
  the same lockin representation problem as the `rafaello.lock` carve-out.

- **Environment handling needs a core-injected-secret distinction.** User
  secrets inherited from the parent env should be scrubbed, but core-issued
  ephemeral credentials like bus tokens or proxy credentials need a safe
  injection path.

- **Tool sink metadata is missing.** CaMeL's policy engine needs to know
  which tools are network sinks, filesystem sinks, mail sinks, git push
  sinks, etc. That metadata is not committed in the v1 security RFC and
  probably belongs in Stream F's manifest schema.

- **Provider-vs-middleware remains unresolved.** The CaMeL RFC raises this
  as an open question, but the rest of the prompt assumes provider is the
  answer. The RFC should either commit to provider for v1 or mark the
  implementation prompt conditional.

- **`core.session.tool_request` as the only LLM-to-tools path must be made
  architectural, not conventional.** The security story depends on core
  refusing direct plugin-to-plugin tool invocation and any alternate RPC
  route that bypasses the provider/policy layer.

- **Install/update digest checks are good but exec-time tamper remains.**
  The RFC lists this as a non-goal. That is acceptable, but implementation
  should ensure the residual risk is visible in user-facing docs when
  plugins are installed from mutable local directories.

## Suggested next revision plan

1. Add a canonical topic grammar and ACL matching section.
2. Rewrite bus authentication/transport to work under lockin, likely with
   pre-opened fds and a reserved core-injected credential path.
3. Expand the lock schema to include manifest-derived runtime authority:
   tools, provider role, topic grants, private state, and sink metadata.
4. Rework filesystem grant compilation around what lockin can actually
   express, or file a lockin extension for deny-subpaths.
5. Clarify v1's exact exfiltration guarantee and add mandatory taint or
   confirmation semantics if the stronger claim is desired.
6. Downgrade CaMeL's "no gap" statement until provider authority,
   confirmation protocol, sandboxed child spawning, and private state
   grants are all part of v1.
