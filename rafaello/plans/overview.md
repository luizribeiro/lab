# rafaello v1 architecture (overview)

> **Status:** ratified 2026-05-08. Three pi review rounds for the
> overview itself (`overview-review-{1,2,3}.md`), three for the
> milestones overview (`milestones/pi-review-{1,2,3}.md`), and
> two for streams A and B; all rounds converged. The 34 rows in
> `decisions.md` are now `ratified` (except row 14, which is
> `reversed` by row 26 — helpers deferred to v2). The next phase
> drafts `milestones/m0-fittings/scope.md`.
>
> This is the single source of truth for v1. If anything in
> `streams/` conflicts with what is written here, this document
> wins and the relevant stream RFC gets a follow-up edit logged in
> the next milestone's retrospective (per `plans/README.md`).

## 0. How to read this

The five stream RFCs (`streams/a-security`, `streams/b-fittings`,
`streams/c-scripting`, `streams/e-renderer`, `streams/f-manifest`)
were authored independently and disagree on naming, scope, and
sometimes on substance. This overview reconciles them and is
deliberately the place where those disagreements get pinned to one
answer. Every reconciliation is called out explicitly in the
section it lives in; the load-bearing decisions are repeated in
§13 so they can be replayed into `decisions.md`.

Each section explains what a subsystem **is**, how it **composes**
with the others, and where it **defers** to a stream. Field-level
specs (TOML keys, JSON-RPC envelopes, exact error variants) live
in the streams; this document does not duplicate them.

## 1. Goals and non-goals

### 1.1 Goals

1. **Project-scoped, single-user.** `rfl init` anchors an agent to
   a directory; `rafaello.lock` lives at the project root. Without
   `rfl init`, `rfl` runs as a tool-less LLM client.
2. **Minimal trusted core.** The Rust binary `rfl` exposes only
   primitives (provider dispatch, tool routing, event bus, session
   store, frontend attach, plugin supervision). Tools, providers,
   renderers, and most user-visible features ship as plugins.
3. **Plugins as the unit of capability.** A plugin manifest is a
   *request*; the lock is the user's *grant*; a kernel sandbox
   (`lockin` for v1) is the *enforcer*. The LLM cannot grant
   itself capabilities.
4. **Cross-tool exfiltration is gated at the bus.** Mandatory
   taint propagation plus core-mediated sink confirmation closes
   verbatim flows; laundered flows (model-rephrased data) are
   explicitly deferred to v2/CaMeL.
5. **Multiple frontends over one bus.** The default ratatui TUI
   is one client; a daemon mode lets web/IDE/email frontends
   attach over JSON-RPC. *(v1 ships TUI only; the daemon /
   external attach pieces are deferred to v2 per `decisions.md`
   rows 27 and 34. The bus model and namespace are sized for
   the full picture from day one.)*
6. **CaMeL-as-plugin is buildable on v1.** Every primitive CaMeL
   needs (provider role, structured taint, sink confirmation,
   fd-based bus, frontend-mediated confirmation) is committed
   in this document. *(v1 commits all of these except helper
   plugins, which were deferred to v2 alongside CaMeL itself
   per `decisions.md` row 26 — CaMeL's Q-LLM isolation runs
   as a plain plugin in v2, with helpers added the same
   release.)*

### 1.2 Non-goals (v1)

- **No embedded scripting language.** Customisation is
  declarative TOML/Markdown plus subprocess plugins. See §7 and
  `streams/c-scripting/rfc-scripting-decision.md`.
- **No VM-level isolation.** Lockin is a process-tree sandbox
  sharing the host kernel. Capsa-VM-per-tool is v2.
- **No multi-tenant isolation.** rafaello is a single-user CLI.
- **No plugin signing or reproducibility checks.** Install-time
  content digest is the only integrity check; mid-run tamper is
  not detected.
- **No anti-laundering of taint.** v1 catches verbatim
  exfiltration; rephrased exfiltration is CaMeL territory.
- **No higher-level sampling/elicitation protocol in v1.** The
  fittings transport supports bidirectional `PeerHandle::call`
  (§4.1, §15.6) — that is the v1 transport-layer commitment.
  But specific human-in-the-loop protocol methods (LSP-style
  sampling, MCP-style elicitation, structured progress
  prompts) are deferred to a follow-up RFC layered on top.
- **No project-type / filetype lazy-load triggers.** rafaello
  has no buffer concept; `ft`-style triggers are deferred until
  there is a "project kind" abstraction in core.

## 2. Trust model and security posture

Three actors, two trust boundaries:

```
+------------------+   trusts   +-----------+
| rfl agent core   | ---------> | the user  |
| (Rust, this repo)|            +-----------+
+------------------+               ^ confirms
        | enforces                 | grants
        v                          |
+------------------+   treats as   +-----------+
| Plugin processes | <------------ |  LLM &    |
| (lockin sandbox) |   hostile     |  its data |
+------------------+               +-----------+
```

- **Agent core** is trusted. It is the only writer of canonical
  `core.*` events on the bus; the only thing that spawns plugins,
  derives policies, and brokers consent.
- **The user** is the only entity that can broaden a grant.
  Grants are explicit and persisted in the lock; the running
  agent and the LLM never mutate it.
- **Plugins** are confined: each runs in its own lockin sandbox
  whose policy is compiled deterministically from its locked
  grant. Plugins talk to each other only through the core-brokered
  bus.
- **The LLM and any byte that has touched the LLM** is treated as
  network input. Even an "own model on own machine" produces
  output that may have been shaped by prompt injection in any
  previous tool result.

The v1 exfiltration claim is **precise**: rafaello v1 prevents
verbatim LLM-mediated cross-tool exfiltration without explicit
user consent. It does not prevent laundered flows, side channels,
or social-engineered overrides. CaMeL-as-plugin (v2) upgrades the
verbatim claim to a capability-system claim.

Authoritative spec: `streams/a-security/rfc-security-model.md`.

## 3. Process model

> **v1 cast (cuts from the full picture below):**
> - The **helper plugin** box and `RFL_HELPER_FD` arrow are
>   **deferred to v2** per `decisions.md` row 26.
> - Of the frontend choices, only the local-spawned `rfl-tui`
>   subprocess exists in v1; the external-attached web / IDE /
>   email row is **deferred to v2** per `decisions.md` row 27,
>   along with the public `rfl serve` command (row 34) and the
>   attach-socket / attach-token plumbing.
> - The **renderer plugin** box ("plug (rndr)") shows where a
>   subprocess renderer would attach in v2; v1 renderers are
>   built-in Rust inside core (`decisions.md` row 29).
>
> The diagram below is the eventual cast; §16 lists the v1
> subset.

A running rafaello session is a small, fixed cast of OS processes:

```
            +---------------------------------+
            |  rfl core (single Rust process) |
            |   - bus broker                  |
            |   - grant compiler              |
            |   - session store               |
            |   - tool router                 |
            |   - confirmation broker         |
            |   - plugin supervisor           |
            |   - renderer cache              |
            +-----+------+----------+---------+
                  |      |          |
       socketpair |      | sp       | sp                socketpair (helper)
                  v      v          v                          v
            +-------+ +-------+ +---------+              +-----------+
            | plug  | | plug  | | provider|<--HELPER_FD->|  helper   |
            | (tool)| | (rndr)| |  plugin |              | (camel-q) |
            +-------+ +-------+ +---------+              +-----------+
                  ^                  ^
                  | TCP UDS          | RFL_BUS_FD
                  |                  | (inherited)
            +-----+--------+
            | frontends    |
            | (TUI, web,   |
            |  IDE, email) |
            +--------------+
```

- **Core** is one Rust binary (`rfl`). It links the bus broker,
  the supervisor, the tool router, and the session store as
  in-process modules. There is no out-of-process "daemon"
  separate from "agent" in v1; v1's only entrypoint is `rfl
  chat`, which runs core + the local TUI in one process tree.
  Public `rfl serve` (and the attach socket it would expose)
  is **deferred to v2** per `decisions.md` row 34.
- **Plugins** are subprocesses spawned by core under lockin.
  Every plugin gets exactly one bus handle (an inherited
  `AF_UNIX SOCK_STREAM` socketpair fd, advertised via
  `RFL_BUS_FD`); the broker side of the pair is bound at spawn
  time to the plugin's authenticated principal.
- **Helper plugins** (§9) are spawned by core on a parent's
  request, *without* an `RFL_BUS_FD`. They communicate with
  their parent over a second inherited socketpair fd
  (`RFL_HELPER_FD`) carrying point-to-point JSON-RPC. Helpers
  cannot speak on the bus.
- **Frontends** are bus principals (§10), **always separate
  processes from core** — there is no in-process frontend in
  v1. The default TUI is a separate `rfl-tui` subprocess
  spawned by core with an inherited bus socketpair like a
  plugin (`RFL_BUS_FD`). External frontends (web, IDE, email)
  attach via a per-session UDS at
  `${XDG_RUNTIME_DIR}/rafaello/<session>/attach.sock` (mode
  `0600`) presenting a one-shot attach token from
  `attach.token` in the same dir.

Trust posture for the cast above:

- Core is the only **trusted** process.
- Plugins (including helpers) are **untrusted, lockin-
  sandboxed**: a separate compiled lockin policy per spawn,
  derived from the lock.
- Frontends are **trusted user-interface principals, not
  lockin-sandboxed plugins**. They speak for the user inside
  the trust model (they answer confirmation requests; security
  RFC §5.7.2 makes this explicit) and therefore are not
  confined by a lockin policy. Their authority on the bus is
  scoped to `frontend.<attach-id>.*` by the broker; they have
  no manifest, no grant block in the lock, and no compiled
  capabilities.
- A user with filesystem access to
  `${XDG_RUNTIME_DIR}/rafaello/` is by definition the user
  (single-user-CLI non-goal); anyone else with that access is
  outside the threat model.

## 4. The bus

The bus is the single communication primitive between core,
plugins, and frontends. It is a **publish/subscribe broker
implemented in core**, on top of point-to-point JSON-RPC
connections that are themselves implemented by `fittings`
(`streams/b-fittings/`).

### 4.1 Two layers, easy to confuse

This is the most common cross-stream confusion, so it is pinned
here:

- **Transport layer (fittings).** Each plugin/frontend has one
  JSON-RPC connection to core, framed `\n`-delimited over its
  inherited fd. fittings owns request/response correlation,
  cancellation, server-side ServiceContext, error mapping,
  and notification flow control. See
  `streams/b-fittings/rfc-fittings-notifications.md` and
  `streams/b-fittings/rfc-fittings-errors.md`.
- **Broker layer (rafaello core).** The bus broker, implemented
  inside core, is what makes "topic", "publish/subscribe ACL",
  "taint envelope", "in_reply_to correlation", and "publisher
  identity" exist. It uses fittings notifications as its wire
  format but is not part of fittings itself.

Concretely: every bus fd is a **bidirectional fittings peer
connection**. Each side runs both a fittings server (handling
inbound requests + notifications from the peer) and a fittings
client (issuing outbound requests + notifications to the peer)
over the same fd. This is the v1 transport primitive; it is a
small but real extension to the v1 fittings cut and is detailed
in §15.6.

The four flow shapes that ride on a peer connection:

- **Plugin → core notification** (e.g. `bus.publish`). The
  plugin's local fittings *client* sends a JSON-RPC
  notification to the core-side server: `Client::notify(
  "bus.publish", { topic, payload, in_reply_to, taint? })`.
  Already in v1 fittings.
- **Plugin → core request** (e.g. `bus.subscribe` confirming
  a runtime subscribe pattern, `helper.spawn`). Plugin's
  client issues a request: `Client::call("bus.subscribe",
  ...).await`. Already in v1 fittings.
- **Core → plugin/frontend notification** (broker fan-out:
  token streaming, `core.session.*` events, confirmation
  requests). Core uses the connection-scoped *server*
  handle: `peer.notify(method, params)`. Outside any
  inbound handler. **New in v1** (was deferred in Stream
  B's "Future work"; promoted by decision 22).
- **Core → plugin/frontend request** (e.g.
  `renderer.render`, `frontend.hello` reverse direction,
  CaMeL helper-spawn synchronous handshake). Core uses the
  same handle: `peer.call(method, params).await`. **New in
  v1** (lifted from Stream B's "Server-originated requests:
  v1 cut", since the bus design and the renderer model both
  require it).

The unified API surface, layered onto Stream B:

```rust
// Each side of a fittings connection holds one of these.
// Cheap to clone; valid for the lifetime of the connection.
impl PeerHandle {
    /// Outbound notification (no response expected).
    /// Shares the bounded notification channel from the
    /// notifications RFC §3b; drop-on-full, non-blocking.
    pub fn notify(&self, method: &str, params: Value)
        -> Result<(), FittingsError>;

    /// Outbound request (response correlated by JsonRpcId).
    /// Pending state is tracked in a per-connection
    /// HashMap<JsonRpcId, oneshot::Sender>, mirror of the
    /// existing client-side correlator.
    pub async fn call(&self, method: &str, params: Value)
        -> Result<Value, FittingsError>;

    /// Resolves when the connection has been closed (peer
    /// shutdown, transport error, local shutdown).
    pub async fn closed(&self);
}

impl<T: Service> Server<T> {
    /// Connection-scoped peer handle. Replaces the round-1
    /// ServerHandle sketch; same notification semantics,
    /// adds outbound `call`.
    pub fn peer(&self) -> PeerHandle;
}
```

Symmetrically, the existing `Client` type gains the inbound
side: `Client::connect(connector)` constructs the client and
`Client::with_service(svc)` registers an inbound `Service`
impl that handles peer-originated requests (replacing the v1
cut's `-32601` reject path). `Client::peer() -> PeerHandle`
exposes outbound notify/call. The two-step
construct-then-register shape (rather than a single
`Client::new(transport, service)` constructor) lets callers
opt into inbound services without forcing every client to
declare one. See §15.6 for the full landed surface.

Authoritative spec: `streams/b-fittings/rfc-fittings-
notifications.md` after the patch tracked in §15.6 of this
overview.

Plugins do not connect to each other; every event is a
core-mediated re-emission.

Reconciliation note: Stream A specifies the *broker* semantics
(publish authority, taint, ACL). Stream B specifies the
*transport* semantics (cancellation, encoding, channel
backpressure). They do not contradict; they layer. Where Stream
B mentions "the bus", read it as "the connection to core";
where Stream A mentions "the bus", read it as "the broker
above". Neither stream owns the wire format end-to-end alone.

### 4.2 Topic and pattern grammar

Authoritative grammar:
`streams/a-security/rfc-security-model.md` §5.1.

- **Topic:** dot-separated segments
  (`segment := [a-z0-9_-]+`), no `:`, no `/`, no in-segment
  wildcards. Tool names, correlation ids, and any other
  discriminator live in the *payload*, never in the topic.
- **Pattern:** dot-separated, with `*` matching exactly one
  segment and `**` (final pseg only) matching one or more
  trailing segments. Patterns and topics are *distinct
  syntactic categories*; an in-pattern `*` is not a valid
  topic.

Reconciliation note: Stream F's manifest schema (`rfc-manifest-
schema.md` §4) showed a glob suffix on a topic
(`fs.changed:**/*.rs`), which uses the `:` and `/` characters
the canonical grammar forbids. **Stream A wins.** File-glob
filtering is a payload concern, not a topic concern; a
plugin watching for Rust source changes subscribes to the
`fs.changed` topic and filters in its handler. Stream F gets
this corrected in the next milestone retrospective; the
example `fs.changed:**/*.rs` in §4 of the manifest RFC must
be read as `fs.changed` with payload-level filtering.

### 4.3 Top-level namespaces

Four publish-authority bands; a publisher that emits outside
its band is rejected at the broker. The `<...>` placeholders
denote **three different id types**, not one — keep them
distinct in implementation:

| Prefix                     | Publisher                           | What `<id>` means here          |
|----------------------------|-------------------------------------|---------------------------------|
| `core.*`                   | agent core only                     | n/a                             |
| `provider.<provider-id>.*` | the bound provider plugin           | `provider-id` (human-readable, lock-bound) |
| `plugin.<topic-id>.*`      | the plugin authenticated on the connection | `topic-id` (hashed plugin id)   |
| `frontend.<attach-id>.*`   | the authenticated frontend          | `attach-id` (human-readable, attach-bound) |

The three id types and their lifetimes:

- **`provider-id`** is the human-readable string a provider
  plugin declares in its manifest as `provides.provider`
  (e.g. `camel`, `anthropic`); it is recorded in the lock's
  `bindings.provider_id` for that plugin (security RFC §3.2).
  Provider ids are user-meaningful and stable across plugin
  versions; they are *not* hashed.
- **`topic-id`** is the only namespace that uses the hashed
  form `id_<base32-no-pad-lower(sha256(canonical-id))[0..16]>`,
  because the canonical plugin id `<source>:<name>@<version>`
  contains `:`, `/`, and `@` which are illegal in topic
  segments. Collision-checked at install time. Authoritative
  spec: `streams/a-security/rfc-security-model.md` §5.1.
- **`attach-id`** is the human-readable id assigned to a
  frontend at attach time (`tui`, `gui`, `ide`, `email`); for
  local-spawned frontends core picks the id, for external
  frontends the frontend proposes one and core refuses
  collisions (security RFC §5.7.1). Attach ids are scoped to
  the session; they do not persist in the lock.

Subscribe authority is per-pattern, granted by the lock and
checked on every delivery.

### 4.4 Provider plugins and the "core re-emit" rule

A provider plugin (the LLM client; e.g. anthropic, openai, the
CaMeL provider) publishes its turn output on its own
namespace, e.g. `provider.camel.tool_request`. **Core observes,
validates, synthesises taint, gates on sink confirmation, and
re-emits the canonical `core.session.tool_request`**. Tool
plugins never see the provider's namespace; they see only
core's canonical events.

The symmetric path applies to results: a tool plugin publishes
`plugin.<topic-id>.tool_result`; core validates `in_reply_to` and the
taint-superset rule, then re-emits canonical
`core.session.tool_result` to subscribers (the bound provider,
frontends, and any other plugin granted the subscription).
Authoritative spec: §5.4 and §5.4.1 of the security RFC.

This is the structural reason *core* is the only writer of
`core.*`: every cross-trust-boundary fan-out is a core decision,
never a direct plugin-to-plugin route.

### 4.5 Bus event envelopes

Every event carries:

- `topic` — the dot-separated topic.
- `payload` — kind-specific JSON object.
- `request_id` (when applicable) — correlation id; type
  `JsonRpcId` (string | number | null), preserved per
  `streams/b-fittings/rfc-fittings-notifications.md` §2a.
- `in_reply_to: [<request_id>...]` — required on every event
  class whose semantics imply taint inheritance (tool results,
  RPC replies, confirm answers, provider tool requests, provider
  assistant messages); see security RFC §7.2.6 for the table.
- `taint: [{ source, detail }, ...]` — structured, mandatory on
  `core.session.tool_result` and `core.session.tool_request`,
  populated by core (§7.2.1–7.2.2 of the security RFC).

Reconciliation note: Stream A's bus envelope adds fields
(`taint`, `in_reply_to`) that Stream B's `ctx.notify` API does
not surface as first-class params. They live inside the
notification's `params` object. Core synthesises and validates
them before fan-out; plugin authors never write `taint` directly
(they may add to it for their own published events, but core
verifies the superset rule).

### 4.6 Reserved env vars

Core injects, and never exposes to user-supplied `env.pass`:

- `RFL_BUS_FD` — inherited fd number for the plugin's bus
  socketpair. Integer, not a secret.
- `RFL_PLUGIN` — the canonical plugin id, for logging and
  error reporting.
- `RFL_HELPER_FD` — for helper plugins (§9), the inherited fd
  for the parent-helper point-to-point channel. Mutually
  exclusive with `RFL_BUS_FD`: helpers have one or the other,
  never both.

Authoritative: security RFC §5.5.1.

## 5. Plugins: manifest, lock, policy, lifecycle

> **v1 status (manifest schema):** rows 30–32 of `decisions.md`
> simplify the v1 manifest:
> - `runtime` field omitted in v1 (default: subprocess+lockin;
>   row 30).
> - `[rpc]` block dropped; the plugin ships a path-referenced
>   `openrpc.json` sibling at the manifest's parent directory
>   (row 31).
> - The compiler emits a **structured `CompiledPlugin` plan**
>   (m1 owns this; m2 applies it to lockin's Rust builder API
>   at spawn time), not a `lockin.toml` artifact — the third
>   arrow in the diagram below is a structured plan handed to
>   m2's supervisor, which applies it to a `SandboxBuilder`
>   per `decisions.md` row 32. m1's draft wording said
>   "lockin builder calls in-memory" — refined in m1
>   retrospective to the plan-based shape that round-1 pi review
>   forced.
>
> **v1 status (helper plugins):** the `helper_for` row in §5.3
> is **deferred to v2** along with the rest of §9 (`decisions.md`
> row 26). v1's lock has no `helper_for` field.

### 5.1 Three artifacts, one direction

```
plugin author writes        rfl install --review writes        runtime applies
+----------------+          +-------------------+              +----------------+
| rafaello.toml  | -------> | rafaello.lock     | -----------> | lockin builder |
| (request)      |  user    | (grant + digest   |  spawn time  | (Rust API,     |
+----------------+  edits   |  + bindings)      |              |  in-memory)    |
                            +-------------------+              +----------------+
```

- The **manifest** (`rafaello.toml` at the plugin root) is the
  plugin author's *request*: methods, subscribed/published
  topics, capability bundles, renderer registrations,
  lazy-load triggers. Authoritative shape:
  `streams/f-manifest/rfc-manifest-schema.md`.
- The **lock** (`rafaello.lock` at the project root) is the
  user's *grant* plus core-computed metadata: a content digest,
  a manifest snapshot digest, the granted subset, and a
  `bindings` block snapshotting manifest-derived authority
  (tool names, provider role, sink classes, renderer kinds —
  `helper_for` was in the round-1 design but is **deferred to
  v2** per `decisions.md` row 26). Mutated only by `rfl install`,
  `rfl grant`,
  `rfl revoke`, and `rfl update`. Authoritative shape: security
  RFC §3.2.
- The **lockin policy** is compiled at every plugin spawn from
  the lock and discarded on plugin exit. Compilation rules in
  security RFC §7 and §8 of the manifest RFC.

The compiler reads only the *lock*. The on-disk manifest is
consulted at install/update time only; the spawn path uses the
lock's bindings snapshot. This closes the rug-pull where a
mutated manifest could change runtime authority without a
re-grant.

### 5.2 Capability scoping

The manifest may split capabilities into sub-bundles
(`[capabilities.default.*]` plus per-method
`[capabilities."<method>".*]`). The lock records the granted
bundles. **In v1, the compiler unions them into a single
spawn-time policy** because lockin does not support live
in-process policy switching; per-method enforcement above the
sandbox layer is core's responsibility (the dispatcher checks
the requested method against the per-method bundle's
declarations before forwarding the call).

Reconciliation note: Stream F leaves this as an open question
(§11 #1, "ship only `default`, or accept scoped bundles with
flatten?"). **Decision: accept scoped bundles, flatten at
compile time.** Per-method enforcement happens in core (above
the sandbox), and the union flatten gives the user a single
policy to reason about. Tightening to true per-method sandbox
policies is a v2 follow-up gated on a lockin feature.

### 5.3 Required manifest fields (v1 commitments)

Stream F's RFC enumerates the schema in detail. The fields
that **Stream A's security model depends on** must all be in
the v1 manifest schema; this is the load-bearing cross-stream
compatibility requirement:

| Field                                    | Purpose                                  | Where |
|------------------------------------------|------------------------------------------|-------|
| `provides.tools = [...]`                 | Tool dispatch routing (§4.4)             | NEW — gap, see §15 |
| `provides.provider = "<id>"` / role flag | Provider role binding (§4.4)             | NEW — gap, see §15 |
| `provides.tool.<n>.sinks = [...]`        | Sink classes for confirmation gate (§6)  | NEW — gap, see §15 |
| `provides.tool.<n>.grant_match`          | User-grants matcher schema (§6.4)        | NEW — gap, see §15 |
| `bus.subscribes` / `bus.publishes`       | Topic ACL                                 | already in F §4 |
| `[capabilities.*]`                        | Compiles to lockin policy                 | already in F §5 |
| `[load]`                                  | Lazy-load triggers                        | already in F §7 |
| `[[renderers]]`                           | Renderer registry (§8)                    | already in F §6 |
| `helper_for` (lock-side `bindings`)       | Helper-plugin parent declaration (§9)     | NEW — gap, see §15 |
| `always_confirm` (per-tool, enforced)     | Force confirmation even when `sinks = []` (UX gate; see §15.1 row 3 below) | NEW — gap, see §15 |

The **NEW** rows are real cross-stream gaps the manifest RFC
has not yet committed to; they are tracked in §15 and must
land in Stream F before m1 implementation begins.

### 5.4 Lazy loading

Five triggers plus `manual` (`eager`, `boot`, `event`,
`command`, `kind`); spec in
`streams/f-manifest/rfc-manifest-schema.md` §7.

Boot sequence (paraphrased):

1. core reads the lock, registers every installed plugin's
   surface (tool names, renderer kinds, subscribe patterns)
   as **stubs** in the routing tables;
2. spawns `eager` plugins, blocks on their handshake;
3. accepts user input; spawns `boot` plugins in parallel;
4. on first event/command/kind hitting a stub, spawns the
   plugin and holds the dispatch until handshake completes.

Eager-plugin handshake failure is **fail-closed in v1**
(security RFC §9 #7): the loop refuses to accept the first
turn until the plugin attaches; the only override is the CLI
flag `rfl plugin start --skip-eager <name>`. There is no
manifest-level "fail open" knob in v1 — pi flagged the earlier
draft's `load.eager_failure` as a speculative schema field
Stream F never committed to. A per-plugin manifest knob is
deferred to v2 pending real usage data.

### 5.5 Per-plugin private state

Every plugin automatically receives a recursive read+write
grant on `${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/`
(the hashed form per `decisions.md` row 5; the raw
`<source>:<name>@<version>` canonical id is not path-safe), with
no manifest request and no user prompt. This is where plugins
persist caches, audit logs, and indexes. Other plugins cannot
read it. The grant **does not count toward
`has_workspace_write`** for trifecta purposes (security RFC
§7.1, §7.2.5). Topic-id form ratified in `decisions.md` row 37
(refines row 16); landed in m1's `compile::compile_plugin` per
scope §C5.

## 6. The grant compiler and tool-call gate

The grant compiler is the core safety-critical component. It
reads the lock, produces a per-plugin **`CompiledPlugin`
structured plan** (consumed by the spawn path via lockin's
Rust API per `decisions.md` row 32 — m1 emits no
`lockin.toml`) and a broker ACL table, and refuses
configurations that violate v1 invariants. Spec:
`streams/a-security/rfc-security-model.md` §7.

### 6.1 Trifecta refusal (per plugin)

For each plugin the compiler computes
`(reads_untrusted, has_outbound, has_workspace_write)` and
refuses if all three hold; the user override is
`rfl grant --i-know-what-im-doing`, which sets a flag visible
in red on every `rfl status`. The graph check is **direct
one-hop only** (security RFC §7.1.1) — not transitive — because
the structural backstop for cross-tool flows is the bus-level
sink confirmation in §6.2 below.

### 6.2 Bus-level sink confirmation (the cross-tool gate)

Mandatory taint propagation on both `core.session.tool_result`
and `core.session.tool_request`, **populated by core, not by
the publisher**, by matching arg values against recently-emitted
result payloads in the same session (security RFC §7.2.1–7.2.2).

The canonical confirmation rule, normative for v1:

> **Any tool_request whose target tool declares one or more
> sink classes is held until a matching `core.session.
> confirm_reply` from a frontend, unless a matching
> `user_grants` entry covers the invocation. The rule is
> independent of whether the args carry taint — taint
> influences the wording and details of the confirmation
> prompt, not whether the prompt fires.**

The carried implications:

- An *untainted* sink call (e.g. the LLM proposes
  `git_push` purely from a user message with no prior tool
  results in scope) still requires confirmation absent
  `user_grants`. This is intentional: the LLM may have
  decided to push for reasons the user did not endorse.
- A sink call whose only taint is `{source: "user"}` also
  requires confirmation absent `user_grants`. Per §6.4
  below, user-data provenance is not user authorisation.
- The `user_grants` table (§6.4) is therefore the only
  way for a sink call to bypass confirmation.

This is the rule pinned by security RFC §7.2.3. Stream A §10
("the v1 summary") still describes the older "non-user taint
AND declared sink" formulation; that summary line must be
patched in the next milestone retrospective to match §7.2.3
and this overview. **Where Stream A §10 disagrees with this
overview, the overview wins** (per `plans/README.md`
§Workflow). The §7.2.3 rule itself is unchanged.

This is the structural fix that broke through pi-A round 2:
the trifecta is gated *at the bus*, not just at the plugin
boundary.

### 6.3 Sinks

Tools declare sink classes in their manifest
(`provides.tool.<n>.sinks = ["network", "vcs_push", ...]`).
Sinks are snapshotted into the lock's
`bindings.tool_meta.<n>.sinks`. Conservative defaults:

| Plugin grant                                | Default sinks              |
|---------------------------------------------|----------------------------|
| `network.mode != "deny"`                    | includes `"network"`       |
| `write_dirs` non-empty (excluding private)  | includes `"workspace_write"` |
| both                                        | both classes               |
| neither                                     | `[]`                       |

A filesystem-writing tool is treated as a sink because tainted
bytes written to the project tree are a vector for later
exfiltration. This explicitly closes the previous "network is
the only sink" framing (pi-A round 2 finding 8).

### 6.4 User grants vs user data provenance

`{source: "user"}` taint is *provenance*, not *authorisation*.
A request whose only taint is user-provenance still requires
sink confirmation unless a matching `user_grants` session-table
entry exists. Entries are populated by:

1. an `rfl` slash command (`/grant send_mail alice@…`);
2. the user answering a confirm with `always_allow_session`;
3. the provider's structured grant proposal, confirmed once.

`user_grants` lives in core memory; process exit clears it. It
is never written to the lock. Spec: security RFC §7.2.4.

### 6.5 Carve-outs by decomposition

`lockin` does not support deny-subpath precedence. The
compiler implements credential-path and lock-file carve-outs
by **refusing or decomposing** ancestor grants (security RFC
§7.3). The carve-out set covers `rafaello.lock`, `.rafaello/`,
`~/.config/rafaello/`, `~/.ssh/`, `~/.gnupg/`, `~/.aws/`,
`~/.config/gh/`, `~/.netrc`. Override:
`rfl grant --allow-credential-paths`, loud in `rfl status`.

### 6.6 Confirmation protocol (core-mediated)

Three core-only topics plus a frontend topic:

- `core.session.confirm_request` (core → frontends)
- `core.session.confirm_reply` (core → requesting plugin)
- `frontend.<attach-id>.confirm_answer` (frontend → core)

Plugins cannot publish replies; frontends cannot publish
replies; only core re-emits a validated answer. Timeout:
60 s default, fail-closed (`deny`). Spec: security RFC §5.6.

## 7. Tool dispatch

A tool call is the only path from LLM-shaped output to a tool
plugin. The path is:

```
provider plugin            -> provider.<provider-id>.tool_request
core (taint + sink gate)   -> core.session.tool_request
                              + plugin.<topic-id>.tool_request
target plugin              -> plugin.<topic-id>.tool_result
core (in_reply_to + taint) -> core.session.tool_result
                              -> provider, frontends, subscribers
```

Tool name → plugin id is resolved from the lock's
`bindings.tools` table; conflicting bindings are a lock-time
error resolved by the user via `rfl provider tool <name>
<plugin>`. There is no plugin-to-plugin RPC route that bypasses
core (security RFC §5.4 closing paragraph).

The **architectural commitment** that makes the security model
hold: `core.session.tool_request` is the *only* path from any
LLM-shaped output to a tool plugin. fittings RPC across plugin
boundaries is expressed as bus events and is therefore subject
to this rule.

## 8. Provider model

A provider plugin is a normal plugin whose lock bindings carry
`provider = true` and a `provider_id`. Multiple providers may be
installed; at most one is **active** per session, recorded in
the lock's `[session]` table as `provider_active = "<plugin-id>"`
(security RFC §3.2). `rfl provider use <id>` is a normal lock
mutation; if the new provider is being granted for the first
time, the install confirmation flow runs.

Providers `subscribe` to `core.session.user_message` and
`core.session.tool_result`, and `publish` on
`provider.<provider-id>.tool_request` and `provider.<provider-id>.assistant_message`.
Core re-emits canonical `core.*` events as in §4.4.

### 8.1 The default provider is a bundled plugin, not a core feature

To preserve the "providers ship as plugins" goal (§1.1) and
the trust model (§2), the default provider is **bundled but
not built-in**: it is a real subprocess plugin named
`rfl-litellm` whose binary ships in the rafaello release and
whose manifest is shipped alongside, but which goes through
the same lock/grant/sandbox/bus path as any third-party
provider.

Concretely:

- `rfl init` materialises `rafaello.lock` with a default
  entry for `rfl-litellm`, pre-populating bindings
  (`provider = true`, `provider_id = "litellm"`),
  `[session].provider_active = "<rfl-litellm-id>"`, and a
  conservative grant: `network.mode = "proxy"` with
  `allow_hosts = ["litellm.thepromisedlan.club"]` (the
  endpoint from `plans/README.md` §"Tooling notes"),
  `env.pass = ["LITELLM_API_KEY"]`, `read_dirs` /
  `write_dirs = []` apart from per-plugin private state.
- The user is prompted at `rfl init` time to confirm the
  default grant (the same `rfl install` review flow), with
  a clearly-labelled "this is the bundled default provider"
  notice. Declining swaps to a tool-less LLM-only
  configuration; the user can later `rfl install` an
  alternate provider.
- Spawn, identity, taint, sink confirmation, and topic
  routing are **identical** to any other provider: lockin
  policy compiled from the lock, bus principal bound at
  spawn, `provider.litellm.*` namespace, core re-emits
  canonical `core.*`. There is no "well-known" path that
  bypasses these.
- The `rfl-litellm` binary lives in the same release crate
  tree but is built and shipped as a plugin artifact, not
  linked into `rfl`. This is a single-binary-source
  release-engineering convenience; it does not change the
  trust model.

Reconciliation note: an earlier overview draft said the
default provider was "built into core". Pi flagged this as
incompatible with §1.1 goals 2–3 and the trust model.
**Decision: bundled-plugin model.** This is the canonical v1
shape; the LiteLLM client code lives in `rfl-litellm`, not in
`rfl`'s core crate.

Third-party providers ship as plugins exactly like
`rfl-litellm`.

## 9. Helper plugins

> **v1 status: deferred to v2** — see `decisions.md` row 26
> (reverses #14). The section below describes the eventual
> design; v1 ships without `bindings.helper_for`, `RFL_HELPER_FD`,
> or the helper spawn path. CaMeL's Q-LLM (the only concrete v1
> consumer) is itself v2; adding helpers later is purely additive
> (no v1 plugin breaks).

Some plugins need a tighter-sandboxed sibling for one job — most
concretely CaMeL's Q-LLM, which must have network egress to the
quarantined model endpoint and *nothing else*. v1 supports this
as a first-class primitive instead of letting plugins wing
subprocess spawning.

A helper plugin:

- is a normal installed plugin with its own digest, manifest,
  lock entry, and lockin policy;
- has `bindings.helper_for = "<parent-id>"` in its lock;
- the parent's lock binding lists which helper ids it may
  spawn (`bindings.helpers = [...]`);
- is **spawned by core** on `provider.<parent-provider-id>.spawn_helper`
  (or `plugin.<parent>.spawn_helper`), not by the parent
  directly;
- runs **without** `RFL_BUS_FD` — it has no bus handle, cannot
  publish, subscribe, or answer confirmations;
- communicates with its parent over a second inherited
  socketpair fd (`RFL_HELPER_FD`) carrying point-to-point
  JSON-RPC, framed identically to the bus but unbrokered;
- has its lifecycle owned by core (SIGTERM on parent exit,
  session end, or `rfl helper stop`).

Spec: security RFC §7.4.1. The helper-channel framing
definition is owed by Stream B (security RFC §9 #5); see §15
of this overview.

## 10. Frontends

> **v1 status: TUI only, no external attach.** v1 ships the
> local-spawned TUI as the only frontend principal (id `tui`,
> publishing on `frontend.tui.*`). The external-attached flavour
> below — UDS attach socket, attach-token handshake, the
> `frontend.<attach-id>.*` namespace beyond `frontend.tui.*`,
> `frontend.hello` capability negotiation — is **deferred to v2**
> per `decisions.md` row 27 (partially reverses #15). Public
> `rfl serve` is also deferred (`decisions.md` row 34); v1's
> `rfl chat` runs core + TUI together in one process tree.

Frontends (TUI, web, IDE, email relay) are first-class bus
principals because they speak for the user inside the trust
model — they answer confirmation requests. Two flavours:

- **Local-spawned.** The default ratatui TUI is spawned by
  core like a plugin: socketpair, `RFL_BUS_FD`, principal
  bound at spawn time. Ids are core-assigned (`tui`, `gui`).
- **External-attached.** *(v2)* A web/IDE/email frontend connects to
  `${XDG_RUNTIME_DIR}/rafaello/<session>/attach.sock` (mode
  `0600`) and presents a one-shot 64-byte attach token from
  `attach.token` in the same dir. Successful handshake binds
  the connection to `frontend.<attach-id>` for the rest of
  the session. Filesystem ACL is the user-level
  authentication.

Frontends may publish on `frontend.<attach-id>.*` only:
`confirm_answer`, `user_message`, plus future-reserved topics.
Core re-emits validated `frontend.*.user_message` as canonical
`core.session.user_message`. *(In v1, `<attach-id>` is always
`tui`.)*

Network-attached frontends (`rfl serve --bind-tcp`) are
designed for but **not enabled** in v1: a TCP listener is an
explicit opt-in with a v2 checklist (security RFC §5.7.2).

Spec: security RFC §5.7.

### 10.1 Frontend capability negotiation

> **v1 status: deferred along with external attach** (`decisions.md`
> row 27). The local-spawned TUI's capabilities are known at compile
> time; core uses static defaults for it. `frontend.hello` returns
> when external attach does in v2.

At attach time the frontend sends a fittings request named
`frontend.hello` carrying its capabilities (renderer-tree
version, color/unicode/scrollback class, image formats,
supported render-tree variants); core uses this to drive
server-side render-tree downgrades (§11).

Spec: `streams/e-renderer/rfc-renderer-model.md` §5.

Reconciliation note: Stream E originally used unprefixed
topic strings (`session.entry.appended`) for streaming entry
notifications. Under the canonical Stream A grammar these
belong inside `core.session.entry.*` because streaming
entries are core-emitted. **Stream A's namespace wins**, and
Stream E was patched in-place during this revision to use
`core.session.entry.*` consistently — see §15.3.

## 11. Renderer model and render tree

> **v1 status (renderers): built-in Rust only.** Subprocess
> plugin renderers — the `renderer.render` JSON-RPC method, the
> daemon-side renderer cache, and any plugin-supplied custom
> kinds — are **deferred to v2** per `decisions.md` row 29
> (partially reverses #19). The render-tree shape, fallback
> rules, and server-side downgrade are kept; only the subprocess
> dispatch is cut.
>
> **v1 status (streaming): `final` only — turn-by-turn replies.**
> The `core.session.entry.appended` and `core.session.entry.patched`
> notifications and the `stream_state: "open"` / `"patch"` enum
> values are **deferred to v2** per `decisions.md` row 28
> (partially reverses #20). v1 emits one `core.session.entry.finalized`
> notification per entry with `stream_state: "final"`.

Conversation history is a sequence of **entries**; rendering
is the pure function `(entry, capabilities) -> RenderTree`
that any frontend can paint. Core stores entries; a renderer
(in-process for built-in kinds, subprocess for plugin-provided
kinds) produces the render tree; the frontend paints it.

Entry shape (Stream E §3): `{id, parent?, kind, schema,
payload, metadata, fallback?}`. Built-in kinds: `text`,
`heading`, `code_block`, `tool_call`, `tool_result`,
`thinking`, `image`, `error`. Plugin kinds are prefixed
(`mermaid:diagram`, `myorg:trace`).

Render-tree variants: a small ADT (~14 nodes) with **no
colour, no layout, no fonts** — purely semantic. The frontend
maps emphasis → ANSI bold or CSS `<strong>`. Variants:
`Text`, `Heading`, `Code`, `Inline`, `Block`, `List`,
`KeyValue`, `Table`, `Divider`, `Image`, `Link`, `Callout`,
`Collapsed`, `Raw`, `Unknown`. Spec:
`streams/e-renderer/rfc-renderer-model.md` §4.

Streaming uses three core-emitted notifications:
`core.session.entry.appended`, `core.session.entry.patched`,
and `core.session.entry.finalized`. Frontends
on append-only surfaces (TUI inline, email) consume only the
`finalized` event for non-text kinds and stream `append_text`
patches directly to stdout; redrawable frontends (web, TUI
alternate-screen) recompute the full render tree on each patch.

Two layers of fallback (Stream E §6) keep frontends dumb: the
entry's author-supplied `fallback.text`/`markdown`, and a
default `Callout { kind: "warn", child: KeyValue { ... } }` if
none provided. Server-side, core downgrades any node a frontend
reported it cannot handle to `Unknown { fallback }` *before*
sending — the frontend never has to invent a fallback.

Subprocess renderers register one JSON-RPC method
(`renderer.render`); the daemon caches results keyed by
`(plugin, kind, payload_hash, caps_hash)` so repaints don't
pay subprocess RTT.

### 11.1 Composition with fittings (cross-stream check)

Stream E's renderer-over-JSON-RPC fits cleanly into Stream B's
notification API:

- `core.session.entry.*` events flow as JSON-RPC notifications
  on each subscriber's bus connection, emitted via the
  connection-scoped server notification handle introduced in
  §4.1. They require no response and benefit from the
  bounded-with-drop notification sink (Stream B §"Notification
  sink"). This is fine because entry patches are advisory
  intermediate frames; the `finalized` event carries the
  authoritative payload.
- `renderer.render` is a request/response method on the
  renderer plugin's own fittings server; cancellation propagates
  via `ctx.cancelled()` if a frontend disconnects mid-render.
- `frontend.hello` is a request/response method on core (the
  frontend's fittings *client* calling core's *server* role).

The one architectural commitment: bus broker fan-out of
`core.session.entry.*` runs at notification rates (token streaming
can be hundreds/sec), so Stream B's drop-on-full sink behaviour
applies. Streaming consumers must tolerate dropped intermediate
patches; the `core.session.entry.finalized` event is the only
authoritative frame.

## 12. Sessions, persistence, daemon mode

A **session** is the unit of conversation history, lock
ownership, attached frontends, and `user_grants`.

- Persistence: SQLite under `${PROJECT_ROOT}/.rafaello/state/`
  storing entries, attached-frontend log, and audit events.
  Plugin private state lives in
  `${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/` (§5.5).
- Branching is **not v1**: sessions are linear in v1 and v2
  inherits the model. The entry schema's `parent` field is
  reserved for v2 branching; v1 leaves it `null`.
- Replay: re-running a session from its entry log is
  supported (the renderer is a pure function of entry +
  capabilities), but interactive replay is post-v1.

**Interactive vs daemon mode.**

> **v1 status: interactive only.** Public `rfl serve` and the
> attach socket are **deferred to v2** per `decisions.md`
> row 34 (and row 27 for external attach). The text below
> describes the eventual two-mode design.

The `rfl` binary always exposes the attach socket. In
**interactive mode** (`rfl chat` in v1; the only mode in v1),
core spawns the bundled `rfl-tui` subprocess and waits on it
for the session lifetime; the TUI is a separate process
attached over an inherited bus socketpair (§3, §10), not an
in-process module. In **daemon mode** (`rfl serve`, v2), core
runs without spawning a TUI and waits for external frontends
to connect to the attach socket. The two modes share one
core process; they differ only in whether core also spawns a
default frontend at startup. A future multi-session daemon
is post-v1.

## 13. Load-bearing decisions (mirror to `decisions.md`)

The architectural calls this overview makes that pi or the
project owner should review before m1.

> **Note:** `decisions.md` rows 26–34 record post-design-phase
> deferrals and refinements. Items 14 (helper plugins), 15
> (frontend principals beyond TUI), 19 (subprocess renderers),
> and 20 (streaming patch ops) below are partially or fully
> reversed by those rows for v1 scope. The full v1 picture
> lives in §16.

1. **No embedded scripting language in v1.** Customisation is
   declarative TOML + Markdown plus subprocess plugins. Source:
   `streams/c-scripting/rfc-scripting-decision.md`.
2. **Lockin is the v1 sandbox; capsa is v2.**
3. **Bus = broker (in core) + transport (fittings).** Two
   layers, not one. §4.1.
4. **Single canonical topic grammar with four namespaces**
   (`core.*`, `provider.<provider-id>.*`,
   `plugin.<topic-id>.*`, `frontend.<attach-id>.*`). §4.2–4.3.
5. **Topic-id form is `id_<base32(sha256(canonical))[0..16]>`**
   with collision rejection at install. §4.3.
6. **Provider plugins publish on `provider.<provider-id>.*`;
   core re-emits canonical `core.*` events.** §4.4.
7. **Mandatory taint on tool_request *and* tool_result;
   structured `{source, detail}`; populated by core.** §4.5,
   §6.2.
8. **Mandatory `in_reply_to` on tool_result, RPC reply,
   confirm_answer, provider tool_request, and provider
   assistant_message.** §4.5.
9. **Bus-level sink confirmation, core-mediated, fail-closed.**
   §6.2.
10. **User-only taint is provenance, not authorisation;
    `user_grants` session table separates the two.** §6.4.
11. **Trifecta graph check is one-hop direct, not transitive.**
    §6.1.
12. **Carve-outs by decomposition, not lockin extension; loud
    `--allow-credential-paths` override.** §6.5.
13. **Bus authentication = inherited socketpair fd
    (`RFL_BUS_FD`); no UDS path, no token.** §3, §4.6.
14. **Helper plugins are a v1 primitive**
    (`bindings.helper_for`, `RFL_HELPER_FD`). §9.
15. **Frontends are first-class bus principals**
    (`frontend.<attach-id>.*`); UDS attach + token for external. §10.
16. **Per-plugin private state is automatic and excluded from
    `has_workspace_write`.** §5.5.
17. **Capability scoped bundles are accepted, flattened at
    compile time; per-method enforcement happens in core, not
    in lockin.** §5.2.
18. **fittings: `Request.id: Option<JsonRpcId>`,
    `Response.id: JsonRpcId`; two channels (unbounded
    response, bounded notification) with drop-on-full;
    bidirectional `PeerHandle` (notify + call, both
    directions over one fd).** §4 and Stream B.
19. **Render-tree is purely semantic (no colour/layout); core
    downgrades unsupported nodes to `Unknown { fallback }`
    server-side.** §11.
20. **Streaming entry topics live under `core.session.entry.*`,
    not unprefixed `session.*`.** §4.2 reconciliation.

## 14. Pi round-2 walkthrough (must-fixes)

The two pi round-2 reviews each listed must-fix items. Claude
addressed them in the streams; pi has not yet returned a round-3
sign-off. This section walks each item and confirms where it
landed, citing section/line in the relevant stream.

### 14.1 Stream A (security) — pi round 2

Reference: `streams/a-security/pi-review-2.md` "Summary: must-fixes".

| # | Pi finding                                                                | Resolution in `rfc-security-model.md`                                                                                            | Status |
|---|---------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|--------|
| 1 | CaMeL RFC stale protocol lines (provider namespace, structured taint, fd-bus, no `/tmp/bus.sock`) | `rfc-camel-on-v1.md` §"What the v1 primitives give you" items 1, 3, 4, 5 — provider namespace ("core re-emits canonical `core.session.tool_request`"), structured taint `{source, detail}`, inherited socketpair `RFL_BUS_FD`, helper plugin replaces `bus.sock` test | **fully addressed** |
| 2 | `bindings.helper_for` go/no-go                                             | Accepted as v1 primitive: `rfc-security-model.md` §7.4.1 fully specifies spawn (by core), comm (`RFL_HELPER_FD`), lifecycle (core-owned, SIGTERM on parent exit), lock fields (`bindings.helper_for`, `bindings.helpers`); also §3.2 example | **fully addressed** (modulo Stream B framing definition — see §15.1 below) |
| 3 | Frontends in bus namespace + auth                                          | `rfc-security-model.md` §5.2 adds `frontend.<id>.*` as fourth namespace; §5.7 specifies attach flow (UDS + one-shot token), trust model (UI principal under user FS ACL), publish authority (`frontend.<id>.*` only) | **fully addressed** |
| 4 | Result-routing path explicit (symmetric to request)                        | `rfc-security-model.md` §5.4.1 spells out the path: `plugin.<id>.tool_result` → core validates `in_reply_to` and taint superset → `core.session.tool_result` → subscribers; failures emit `core.lifecycle.tool_result_rejected` | **fully addressed** |
| 5 | User-only-taint sink bypass replaced                                       | `rfc-security-model.md` §7.2.4 separates user-data provenance from user-authorisation; introduces `user_grants` session table, populated by `/grant` slash commands, `always_allow_session` answers, or provider-extracted proposals; user-only taint is NOT a confirmation bypass | **fully addressed** |
| 6 | `in_reply_to` mandatory where taint inherits                               | `rfc-security-model.md` §7.2.6 table makes `in_reply_to` required on `plugin.<id>.tool_result`, `provider.<id>.tool_request`, `provider.<id>.assistant_message`, `plugin.<a>.rpc_reply`, `frontend.<id>.confirm_answer`; broker rejects missing or violating events | **fully addressed** |

The security RFC's own §9.2 contains the same mapping (rows
1–14) cross-referenced against pi-2's full finding list (not
just the must-fix subset). Every row is resolved by a textual
change in `rfc-security-model.md`, not deferred.

### 14.2 Stream B (fittings) — pi round 2

Reference: `streams/b-fittings/pi-review-2.md` "Summary: must-
fixes".

| # | Pi finding                                                                  | Resolution                                                                                                                        | Status |
|---|-----------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------|--------|
| 1 | Bounded-channel/semaphore deadlock                                          | `rfc-fittings-notifications.md` §3a: dispatcher MUST NOT block on semaphore; specifies refactor (worker-spawner task or `JoinSet`) and includes a normative deadlock-regression test (`notification_capacity=1`, `max_in_flight=1`, two notifications + complete) | **fully addressed** (design); implementation owes the refactor |
| 2 | One canonical channel type                                                  | `rfc-fittings-notifications.md` §3b: two separate channels — `response_tx: mpsc::Sender<Vec<u8>>` unbounded, `notification_tx: mpsc::Sender<Vec<u8>>` bounded (default 1024) with drop-on-full. The remaining `mpsc::UnboundedSender` references at L42 and L201 describe the *current* (pre-refactor) code, not the new design | **fully addressed** |
| 3 | `Request.id` semantics for notifications and `id: null`                     | `rfc-fittings-notifications.md` §2a: `Request.id: Option<JsonRpcId>` (None = inbound notification, Some = inbound request); `Response.id: JsonRpcId` (required; framework discards for notification handlers); explicit rules for `id: null` (treated as a request keyed on `JsonRpcId::Null`, at-most-one in flight) | **fully addressed** |
| 4 | Predefined errors round-trip without information loss                       | `rfc-fittings-errors.md` §A: every predefined variant carries `data: Option<Value>` (`Parse`, `InvalidRequest`, `InvalidParams`, `MethodNotFound`, `Internal` — and `MethodNotFound` also carries `method`); §B.1 outbound preservation table; §C decoder preserves message+data verbatim. Acceptance criterion in §"Acceptance criteria" makes byte-equal round-trip a test | **fully addressed** |
| 5 | Malformed cancellation handling + batch cancellation semantics              | `rfc-fittings-notifications.md` §"Cancellation response semantics" rule 6 (malformed payloads logged at `warn!`, dropped, connection not torn down); §"Cancellation in batch requests" rules 1–6 (per-item `in_flight`, fast-path inside batches, batch-response array shorter on cancel, one semaphore permit per batch) | **fully addressed** |
| 6 | `Cancelled` suppression rules                                               | `rfc-fittings-notifications.md` §"Cancellation response semantics" rules 1, 2, 6: suppression triggers on **either** the token firing or the handler returning `Err(FittingsError::Cancelled)`; `rfc-fittings-errors.md` §"Open questions #5 (resolved)" repeats the rule | **fully addressed** |

All six are resolved in text; pi has not formally signed off.
None are partial.

### 14.3 Pi round-2 non-must-fix findings

Stream A pi-2 also raised findings 7–14 (all minor), and
Stream B pi-2 raised non-must findings 7, 11, 12, 17, 18.
Each is mapped to a textual fix in the corresponding RFC's
"Resolved disagreements" section (security RFC §9.2; fittings
notifications RFC §"Pi-review-2 finding 11" inlined; fittings
errors RFC §"Open questions #5 (resolved)"). Item 17 (the
`tokio::spawn(handler(method, params))` snippet) is corrected
in `rfc-fittings-notifications.md` §6 ("Execution model: spawn
per notification") with the wrapped `async move { handler(...);
}` form.

## 15. Cross-stream gaps and proposed fixes

> **v1 status:** rows 26–32 of `decisions.md` change which of
> the gaps below are still in scope for m1. Specifically:
> - `helper_for` (the row tagged "(§9)" in §15.1) — **deferred
>   to v2** along with the rest of §9 (row 26). m1 does not
>   need this manifest field.
> - Subprocess `renderer.render` and the Stream B "renderer
>   JSON-RPC vs notification API" subsection (§15.5) — the
>   subprocess renderer dispatch is **deferred to v2** (row 29).
>   §15.5's reconciliation still applies if/when subprocess
>   renderers return.
> - Streaming entry topics (`core.session.entry.appended` /
>   `patched`) — only the `finalized` event is in v1 scope
>   (row 28); `appended` / `patched` reconciliation deferred.
> - Manifest fields `runtime` and `[rpc]` — v1 omits them
>   (rows 30 and 31). Stream F's eventual schema keeps them.
>
> The remaining items below are still load-bearing for m1.

### 15.1 Stream F manifest does not yet expose every Stream A field

The security model depends on manifest fields the manifest
schema RFC has not yet committed:

| Field                                | Used by                           | Stream F status |
|--------------------------------------|-----------------------------------|-----------------|
| `provides.tools = [...]`             | tool routing (overview §7)        | absent — F has `[rpc]` and `[[renderers]]` but no `provides.tools` table |
| `provides.provider = "<id>"`         | provider role (§8)                | absent — F has no provider concept |
| `provides.tool.<n>.sinks = [...]`    | sink-class confirmation (§6.2)    | absent |
| `provides.tool.<n>.grant_match`      | `user_grants` matcher (§6.4)      | absent |
| `helper_for` (lock-side binding)     | helper plugins (§9)               | absent — F's open question #1 unrelated |
| `always_confirm` (per-tool, enforced) | Force confirmation even when not a sink | absent |

**Normative delta (binding for Stream F's next revision).**
Pi flagged that the loose "proposed fix" wording was
insufficient for ratification. The following is the precise
schema delta Stream F must adopt before m1 implementation:

1. **Add a `[provides]` block** at the manifest top level:

   ```toml
   [provides]
   tools    = ["grep", "rg"]                # zero or more tool names
   provider = "litellm"                     # absent if not a provider; one provider-id per plugin
   helpers  = ["github:org/qllm@1.0.0"]     # canonical ids of helpers this plugin may spawn

   [provides.tool.grep]
   sinks            = []              # zero or more of: "network", "vcs_push", "mail", "workspace_write", "exec", custom strings
   grant_match      = "schemas/grep-grant-match.json"  # JSON-Schema for user_grants matcher; absent → "any invocation" matcher
   always_confirm   = false           # enforced opt-in: when true, core gates the tool through confirmation even if sinks = []
   ```

   The `[provides]` block is the manifest-side declaration;
   `rfl install` snapshots it into the lock's
   `bindings.tools`, `bindings.provider_id`,
   `bindings.tool_meta.<name>.{sinks,grant_match,always_confirm}`,
   and `bindings.helpers`.

   **Renaming note (pi review 2 finding 2):** the round-1
   draft used `requires_confirmation` and described it as
   "advisory hint". That conflicted with §5.3 calling it
   "Hint, not enforcement" while the validation rules below
   said it was core-enforced. **Decision: enforced opt-in,
   renamed to `always_confirm` for clarity.** It is a UX
   gate (forces a confirmation prompt) layered on top of the
   sink-confirmation rule (§6.2); it is *not* a security
   sink class itself. The advisory-hint flavour is dropped
   entirely — manifest authors who want a hint without
   enforcement can put it in `description`.

2. **Add a top-level `helper_for` manifest field**:

   ```toml
   helper_for = "github:anthropic/camel@0.1.0"   # canonical id of the parent plugin
   ```

   Snapshotted to `bindings.helper_for` in the lock. Mutually
   exclusive with `[provides.provider]`: a helper cannot be a
   provider in the same manifest.

3. **Validation rules at install time:**
   - A plugin with `[provides.provider]` must declare at
     least one `[capabilities.<bundle>.network]` entry, or
     `rfl install` warns.
   - Every entry in `[provides.tools]` must have a matching
     `[provides.tool.<name>]` table; missing tables default
     to `{ sinks = [], grant_match = absent, always_confirm = false }`.
   - `helper_for` referencing a plugin whose lock entry does
     not list this helper in `bindings.helpers` is rejected
     at install time.
   - `always_confirm = true` causes core to gate the tool
     through the confirmation protocol (§6.6) even when its
     `sinks = []` and no `user_grants` entry is involved;
     this is the explicit opt-in for read-only tools that
     nonetheless want a human review step. Bypass via the
     same `user_grants` mechanism as sink confirmation.

4. **Eager-failure mode field — STRIKE.** The earlier overview
   draft mentioned `load.eager_failure = "open"` as a knob.
   Stream F has no such field and pi flagged it as
   speculative. **Decision: defer this knob to v2.** v1 is
   fail-closed-only on eager-plugin handshake, with a
   per-plugin override only via `rfl plugin start --skip-eager
   <name>` at the CLI. Overview §5.4 is patched accordingly.

5. **Topic example fix (already applied to the RFC).** The
   `fs.changed:**/*.rs` example in
   `rfc-manifest-schema.md` §4 / §9.1 / §11 has been
   rewritten to plain `core.fs.changed`; payload-level
   filtering is the plugin's responsibility. This was a
   documentation fix, applied directly during this revision.

6. **Manifest filename harmonised (already applied).** The
   security RFC's stale `plugin.toml` references
   (`rfc-security-model.md` §3.1, §4) have been replaced
   with `rafaello.toml` to match Stream F.

7. **Capability-field naming aligned.** Stream F's
   `[capabilities]` block uses `read_paths`, `read_dirs`,
   `write_dirs`, `exec_paths`, `exec_dirs`. Stream A's lock
   excerpt uses the same names (§3.2). The earlier mention of
   `env_pass` (one word) in Stream A is a typo for the
   manifest's `env.pass` (with dot); the compiler treats them
   identically. No schema change needed.

Items 1–4 are real Stream F work for m1; items 5–7 were
documentation fixes applied in this revision (commit
`docs(rafaello-overview): manifest documentation harmonisation`).

### 15.2 Schema ownership: Stream A owns broker envelopes; Stream B owns JSON-RPC framing

Pi flagged a real contradiction in the round-1 draft: the
overview said Stream A owns bus-level schemas while security
RFC §9 item 3 said Stream B owes them. **Canonical split,
binding for v1:**

| Layer                          | Owner    | Concretely                                                                                                  |
|--------------------------------|----------|-------------------------------------------------------------------------------------------------------------|
| JSON-RPC 2.0 envelopes         | Stream B | `RequestEnvelope`, `ResponseEnvelope`, `ErrorEnvelope`, `JsonRpcId` type, predefined error codes/data fields |
| Wire framing                   | Stream B | `\n`-delimited line framing on bus / helper / attach connections; one-shot `socketpair`                     |
| Connection authentication      | Stream A | Inherited fd binding, principal namespace assignment, attach-token handshake                                 |
| Topic grammar + namespace ACL  | Stream A | Topic / pattern grammar (§4.2), four-namespace publish authority (§4.3), broker subscribe matching          |
| Bus-event payload schemas      | Stream A | Schemas for `core.session.tool_request`, `core.session.tool_result`, `core.session.confirm_*`, `core.session.user_message`, `core.session.entry.*`, `provider.<id>.tool_request`, `frontend.<attach-id>.confirm_answer`, etc. — including the `taint`, `in_reply_to`, `request_id`, and `topic` fields |
| Fan-out / re-emission          | Stream A | Provider-namespace → core-namespace re-emission rules (§4.4), result-routing path (§5.4.1)                  |

Implementation note: security RFC §9 item 3 was patched
in-place during this revision (commit
`docs(rafaello-overview): pin bus schema ownership split
(A=payloads, B=envelopes)`) to read "Stream A owns the bus
event payload schemas; Stream B owns the JSON-RPC envelopes
that carry them." Stream B's schema crates host the JSON-RPC
envelope shape; Stream A's RFC is where `taint`,
`in_reply_to`, and topic-payload schemas are specified.

What Stream B *does* additionally owe (carried from security
RFC §9 #5):

- **Helper-channel framing definition.** `RFL_HELPER_FD`
  carries point-to-point JSON-RPC, framed identically to the
  bus path (`\n`-delimited, JSON-RPC 2.0 envelopes). Stream B
  should add a one-paragraph reference in
  `rfc-fittings-notifications.md` clarifying that the framing
  is reused for unbrokered helper channels.
- **Connection-scoped server notification handle.** See §15.6
  below — required to make the bus broker fan-out
  implementable.

### 15.3 Stream E topic spelling

Stream E originally used unprefixed `session.entry.*` topics;
the canonical namespace requires `core.session.entry.*`
because streaming entries are core-emitted (entries are
validated and canonicalised by core before fan-out). The
rename was applied in-place during this revision (commit
`docs(rafaello-stream-e): use core.session.entry.* spelling
consistently`); Stream E is now consistent with overview.

### 15.4 ServiceContext shape vs bus needs

Stream A's bus envelope adds `taint` and `in_reply_to`; Stream
B's `ctx.notify(method, params)` API does not surface them as
first-class. **Decision: they live inside `params`** as a
JSON-level convention. Core's broker validates and synthesises
them; plugin authors add to `taint` for their published events
but the core enforcer is the source of truth. No fittings API
change is required.

### 15.5 Renderer JSON-RPC vs notification API

Renderer registration uses fittings request/response
(`renderer.render`); streaming entry events use fittings
notifications (`core.session.entry.*`). Both fit cleanly inside
Stream B's API as it stands once §15.6 lands; renderer cache
invalidation on plugin reload is core's job, not fittings'.

### 15.6 Bidirectional fittings peer is a v1 primitive

Pi-review-1 finding 1 promoted connection-scoped
`ServerHandle::notify` to v1. Pi-review-2 blocking finding 1
showed that **notifications alone are not enough**: the bus
design (`renderer.render`, `frontend.hello` reverse direction,
helper-spawn handshake, future tool-call request/response
modes) needs full bidirectional request/response over the
same fd. Stream B v1 still defers server-originated requests.

**Decision: lift the v1 cut on server-originated requests and
ship a unified `PeerHandle` exposing both `notify` and `call`
on the same connection.** This generalises the round-1
`ServerHandle::notify` decision; both are now sub-cases of one
peer surface.

Required Stream B follow-up edits (m1's stream-B brief; the
patches landed in this revision under
`docs(rafaello-stream-b): promote bidirectional peer to v1`):

1. **Replace** the round-1 `Server::handle() -> ServerHandle`
   sketch with `Server::peer() -> PeerHandle` and a matching
   `Client::peer() -> PeerHandle`. Both types expose the same
   `notify` / `call` / `closed` surface.
2. **Add outbound `call` to the server side.** Track pending
   responses in a per-connection
   `HashMap<JsonRpcId, oneshot::Sender>`, mirror of the
   existing client-side correlator. The dispatcher already
   owns `in_flight: HashMap<JsonRpcId, CancellationToken>`;
   `pending_outbound` is a sibling map.
3. **Add inbound request handling to the client side.** The
   client's `select!` loop already tolerates inbound
   id-bearing frames by sending `-32601`; replace that branch
   with a dispatch into a user-supplied `Service` impl when
   one is registered (`Client::with_service(svc)`), preserving
   the `-32601` fallback when no service is registered.
4. **Reuse existing channel infrastructure.** `peer.notify`
   shares the bounded notification channel (notifications RFC
   §3b, drop-on-full). `peer.call` writes into the unbounded
   response channel (responses are bounded by
   `max_in_flight + max_outbound_pending`, so unbounded-writer
   semantics remain safe); reads correlate via the new
   pending map.
5. **Specify request-id allocation.** Each side of a peer
   connection allocates `JsonRpcId`s from its own monotonic
   counter; collisions between directions are impossible
   because each direction tracks pending state independently.
   (Inbound and outbound share the wire, but the response-id
   space is partitioned by who is correlating: if I sent
   request id 7, only my pending map cares about response
   id 7.)
6. **Connection-closed behaviour.** Both `peer.notify` and
   `peer.call` return `Err(FittingsError::Transport { ... })`
   once the writer task observes the connection closed.
   `peer.closed()` resolves on close.
7. **Acceptance criteria.** Two new tests:
   - core→plugin `peer.call` round-trip outside any inbound
     handler (the renderer test);
   - simultaneous in-flight calls in both directions (plugin
     calling `bus.publish` while core is calling
     `renderer.render`).

This is a more substantial change than the round-1
`ServerHandle::notify`, but it is still **non-breaking on the
`Service` trait**: existing handlers continue to compile
unchanged. The added surface is `Server::peer()`,
`Client::with_service()`, `Client::peer()`, and `PeerHandle`;
the `-32601` reject path is replaced by a dispatch when a
service is registered. The scope fits one PR series alongside
the other v1 fittings work.

(The `ServerHandle` name from round-1 is dropped in favour of
`PeerHandle`, since the type is symmetric across both ends of
the connection. Decision 22 in `decisions.md` is amended
accordingly.)

## 16. v1 scope cut

**In v1.** Lockin sandbox; manifest+lock+policy pipeline;
bus broker with four namespaces and structured taint;
core-mediated sink confirmation with `user_grants`; carve-out
by decomposition with loud override; default ratatui TUI as a
local-spawned bus principal (single frontend, no external
attach); CLI subcommands `rfl init / install / grant / revoke
/ update / provider use / status / chat` (no `rfl serve` in
v1 — see `decisions.md` row 34); bundled `rfl-litellm` default
provider plugin (§8.1); renderer model with semantic
render-tree and server-side downgrade, **built-in Rust
renderers only** (no subprocess plugin renderers in v1);
turn-by-turn entries (`stream_state: "final"` only — patch ops
deferred); entry persistence in SQLite; lazy loading with five
triggers + manual; declarative config (TOML+Markdown); fittings
v1 with `ServiceContext`, bidirectional `PeerHandle` (notify +
call in both directions, §15.6), cancellation, two-channel
server loop, predefined error preservation.

**Deferred to v2.**

| Feature                                       | Why deferred                                         |
|-----------------------------------------------|------------------------------------------------------|
| CaMeL provider + Q-LLM helper                 | Heavy; v1 ships the primitives, v2 ships the plugin  |
| **Helper plugins (`bindings.helper_for`, `RFL_HELPER_FD`)** | No v1 consumer; CaMeL is itself v2. Forward-compat: purely additive. See `decisions.md` row 26. |
| **External UDS-attached frontend principals** | v1 ships TUI only, local-spawned. `frontend.<attach-id>.*` namespace reserved. See `decisions.md` row 27. |
| **Streaming entry patch ops (`stream_state: "open"`/`"patch"`)** | v1 emits `final` only (turn-by-turn). Adding `open`/`patch` later is purely additive. See `decisions.md` row 28. |
| **Subprocess plugin renderers (`renderer.render`)** | v1 renderers are built-in Rust, compile-time registered. Plugin authors wait for v2. See `decisions.md` row 29. |
| Capsa-VM-per-tool isolation                   | Capsa not yet ready; lockin is sufficient for v1     |
| Plugin signing / reproducibility              | Adds infra (key management, build tooling) without changing the threat model in v1 |
| Embedded scripting (Luau or other)            | See §1.2; can be added later, removing later is hard |
| Branching sessions                            | Linear sessions cover demos; branching needs UX work |
| **Public `rfl serve` (any flavour)**          | With external attach deferred (row 27), no v1 consumer for an attach socket. v1's only entrypoint is `rfl chat`. See `decisions.md` row 34. |
| Multi-session daemon                          | Re-opens once `rfl serve` returns in v2.             |
| Project-type lazy-load triggers (`ft` analog) | Needs "project kind" abstraction in core             |
| Network-attached frontends (TCP, `--bind-tcp`)| All external attach is v2; TCP is the second step.   |
| Dynamic capability scoping in the sandbox     | lockin can't switch policies live; core enforces above the sandbox in v1 |
| Per-method spawn-time policies                | Same — flatten in v1, revisit on lockin feature      |
| `decisions.md` and `glossary.md` ratification | Pi review pending                                    |

**Outside the sandbox, ever** (non-goals, not deferred):

- hardware-level isolation (kernel exploits, side channels);
- network *content* inspection (we allowlist hostnames, not
  payloads);
- multi-user / multi-tenant isolation;
- a separate policy DSL (the manifest is the policy language).

## 17. Open follow-ups before m1 ratification

Beyond the cross-stream gaps in §15:

1. **Eager-plugin failure mode default** (security RFC §9 #7).
   Overview §5.4 picks fail-closed; pi may want this surfaced
   to the user as a `rfl status` indicator on every boot.
2. **DNS rebinding via `allow_hosts`** (security RFC §6.7).
   Filed as residual risk pending a lockin feature to deny
   private/loopback/link-local/cloud-metadata resolutions in
   `proxy` mode. Not a v1 mitigation.
3. **Mutable-local plugin source warning** (security RFC §9
   #6). `rfl install` and `rfl status` must label
   `source: local-mutable` plugins so exec-time tamper is
   visible.
4. **Renderer priority tie-breaking** (manifest RFC §11 #6).
   Stream E should pin (insertion order? lex order? user
   preference list?). Overview leaves this to Stream E.
5. **Secrets sigil semantics** (manifest RFC §11 #4).
   `${secret:<name>}` resolves against rafaello's keystore at
   spawn time; the keystore is a future stream — not blocking
   v1 unless a built-in provider needs it (the default provider
   reads `LITELLM_API_KEY` from env directly, not via the
   sigil).
6. **Manifest signing** (manifest RFC §11 #3). Deferred to v2
   per §16.

## 18. Reference index

For field-level specs, defer to the streams:

- `streams/a-security/rfc-security-model.md` — trust model,
  bus ACL, grant compiler, taint, sink confirmation, helper
  plugins, frontends, attack scenarios.
- `streams/a-security/rfc-camel-on-v1.md` — v2 prompt,
  v1-primitive dependencies row by row.
- `streams/b-fittings/rfc-fittings-notifications.md` —
  `ServiceContext`, cancellation, two-channel server loop,
  notification sink, `JsonRpcId`, batch cancellation,
  client-side notification handler.
- `streams/b-fittings/rfc-fittings-errors.md` —
  `FittingsError` shape, wire-code policy, predefined error
  preservation, panic policy, middleware contract.
- `streams/c-scripting/rfc-scripting-decision.md` — why no
  embedded scripting in v1; declarative config surface;
  conditional re-evaluation criteria.
- `streams/e-renderer/rfc-renderer-model.md` — entry schema,
  render-tree ADT, capabilities, fallback, streaming,
  subprocess renderers, versioning.
- `streams/f-manifest/rfc-manifest-schema.md` — manifest TOML
  schema, capability bundles, lazy-load triggers,
  compilation to lockin, capsa back-compat story.

Stream-level pi reviews (`pi-review-1.md`, `pi-review-2.md`)
are authoritative records of contested decisions and round-2
fixes; this overview's §14 is a walkthrough, not a substitute.
