# Glossary

One-line definitions for load-bearing terminology. When a term in this
glossary appears in any other doc under `plans/`, it should mean
exactly what is written here. If a doc uses a term in a way that
disagrees with the glossary, fix the doc — not the glossary.

Add a row when a new load-bearing term first lands in `overview.md` or
a `streams/` RFC. Keep definitions to one line where possible.

| Term | Definition |
|------|------------|
| Agent core (`rfl`) | The single trusted Rust process; hosts the bus broker, grant compiler, plugin supervisor, session store, tool router, and renderer cache. |
| Attach socket | Per-session UDS at `${XDG_RUNTIME_DIR}/rafaello/<session>/attach.sock` (mode `0600`) where external frontends connect with a one-shot attach token. |
| Bindings (lock) | Snapshot of manifest-derived runtime authority — tool names, provider role, sink classes, renderer kinds, helper relationships — copied into `rafaello.lock` at install/update time; the spawn path reads bindings, never the live manifest. |
| Bus | The publish/subscribe broker implemented inside core, layered on top of fittings JSON-RPC connections; carries topics, taint, and `in_reply_to` correlation. |
| Bus broker | The component of core that authenticates publishers, enforces topic ACLs, synthesises taint, validates `in_reply_to`, and fans out events to subscribers. |
| Canonical `core.*` event | A core-emitted event on the `core.*` namespace, produced by re-emission from a provider's or plugin's namespaced equivalent after validation. |
| Capability (lockin) | An OS-level grant compiled from the lock — `read_dirs`, `write_dirs`, `exec_paths`, `network.mode`/`allow_hosts`, `env.pass`, resource limits — enforced by the lockin sandbox. |
| Capability bundle | A named group of capabilities in the manifest (`default` plus optional per-method `<method>` bundles); v1 unions them at compile time. |
| Capsa | The future VM-level isolation backend (v2/v3); not used in v1. |
| Carve-out | A path or path class (lock file, credentials, dotfiles) excluded from broad recursive grants; implemented by compile-time decomposition because lockin lacks deny-subpath precedence. |
| CaMeL | The dual-LLM defence pattern (arXiv:2503.18813); shipped in v2 as a provider plugin (`camel`) plus a `camel-qllm` helper. Not in v1. |
| Confirmation protocol | Core-mediated topic family — `core.session.confirm_request` (gate publish on hold), `frontend.<id>.confirm_answer` (frontend publish on user action), `core.session.confirm_reply` (re-emit's canonicalised reply after answer-enum validation), and `core.session.confirm_resolved` (gate publish, only on grant-short-circuit queue pruning) — used to obtain explicit user consent for sink calls and other gated actions. Backed by `ConfirmState`, an atomic state machine over `(Active / TimedOut / ResolvedByAnswer)` entries. The three-arm TUI answer enum is `allow` / `deny` / `always_allow_session`; `timeout` is a gate-side 60s deadline outcome, not a TUI answer. m5a (`decisions.md` rows 46, 47). |
| Audit log | The `audit_events` SQLite table introduced in m5a c08, co-resident in `${PROJECT_ROOT}/.rafaello/state/session.sqlite` alongside `entries`. Authoritative audit-kind enumeration lives at `rafaello-core/src/audit/mod.rs::AuditKind::as_str()`; kinds cover gate dispatch outcomes, the confirmation lifecycle, grant lifecycle (`grant_added` / `grant_revoked` / `grant_list`), slash command outcomes, install-time outcomes (`install_accepted` / `install_refused` / `trifecta_overridden` / `credential_paths_overridden`), and the m5b additions `confirm_request_taint_attached` / `plugin_publish_rejected_taint_superset` / `tool_request_taint_unioned_from_in_reply_to`. |
| Daemon mode | `rfl serve` — the same `rfl` binary running with the attach socket exposed; not a separate process. |
| Entry | One element of conversation history (`{id, parent?, kind, schema, payload, metadata, fallback?}`); persisted in the session store and rendered into a `RenderTree`. |
| Fallback | An entry's author-supplied plain-text/markdown/summary representation, used when no renderer is available or a render-tree node is unsupported. |
| fittings | The in-tree JSON-RPC framework (`fittings-core`, `-server`, `-client`, `-macros`, `-wire`); provides per-connection request/response, notifications, cancellation, and `ServiceContext`. |
| Frontend | A user-facing client (TUI, web, IDE, email relay) that subscribes to bus events and answers confirmation requests; first-class bus principal under `frontend.<id>.*`. |
| Frontend principal | The bus identity assigned to an attached frontend; bound at attach time via inherited fd (local) or attach token (external). |
| Grant | The lock's record of what the user authorised — a subset of the manifest's request, plus core-computed digest and bindings. |
| Grant compiler | The core component that reads `rafaello.lock` and produces (a) per-plugin lockin policies and (b) the broker ACL table; enforces invariants the manifest schema cannot. |
| Helper plugin | A normal plugin whose lock binding declares `helper_for = "<parent-id>"`; spawned by core without `RFL_BUS_FD`, communicating with its parent over `RFL_HELPER_FD`. |
| `in_reply_to` | Correlation field on bus events — `[<request_id>, ...]` — required on event classes that inherit taint (tool results, RPC replies, confirm answers, provider tool requests / assistant messages). |
| Kind (entry) | Routing key on an entry payload (`text`, `code_block`, `tool_call`, plugin-prefixed `mermaid:diagram`); selects the renderer. |
| Lazy-load trigger | A condition that causes a plugin to be spawned (`eager`, `boot`, `event`, `command`, `kind`, `manual`); declared in the manifest's `[load]` block. |
| Lock (`rafaello.lock`) | Project-root TOML file recording, per installed plugin, the granted capabilities + content digest + manifest-snapshot digest + bindings; mutated only by `rfl` install/grant/revoke/update. |
| Lockin | The OS-level process-tree sandbox used in v1; consumes a per-spawn policy applied via lockin's Rust builder API at spawn time (`decisions.md` row 32 — no `lockin.toml` artifact in v1; m1's `CompiledPlugin` plan is the structured source). |
| Manifest (`rafaello.toml`) | Plugin author's request, shipped at the plugin root; declares identity, methods, subscribed/published topics, capability bundles, renderer registrations, lazy-load triggers. |
| Per-plugin private state | `${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/` (the hashed form per `decisions.md` row 5; canonical id is not path-safe — `decisions.md` row 37 refines row 16); recursively read+write granted automatically; excluded from `has_workspace_write`. |
| Pattern (topic) | A subscribe pattern using `*` (one segment) or `**` (final, one or more trailing segments) on top of the topic grammar; distinct syntactic category from a topic. |
| Plugin id, canonical | `<source>:<name>@<version>` (e.g. `github:acme/grep@1.4.2`); retained in the lock and human-facing logs. |
| Plugin id, topic-id | `id_<base32-no-pad-lower(sha256(canonical))[0..16]>`; the form used inside bus topic segments. Collision-checked at install. |
| Provider plugin | A plugin whose lock bindings carry `provider = true` and a `provider_id`; publishes on `provider.<id>.*`; at most one is active per session. |
| RenderTree | Small semantic ADT (`Text`, `Block`, `List`, `Table`, `Code`, `Callout`, `Collapsed`, `Unknown`, …) describing what an entry *means*, with no colour/layout/font; frontends paint it. |
| `RFL_BUS_FD` | Reserved env var pointing to the inherited socketpair fd that is a plugin's only bus handle. |
| `RFL_HELPER_FD` | Reserved env var pointing to the inherited socketpair fd connecting a helper plugin to its parent; helpers have this *instead of* `RFL_BUS_FD`. |
| `RFL_PLUGIN` | Reserved env var carrying the canonical plugin id, for the plugin's own logging. |
| Round-2 must-fix | A finding from `pi-review-2.md` flagged as blocking implementation handoff; addressed in the corresponding stream RFC's "Resolved disagreements" section. |
| Sandbox policy | The compiled, ephemeral lockin builder calls (or capsa equivalent) materialised on every plugin spawn from m1's `CompiledPlugin` structured plan and discarded on exit; never hand-edited. (Pre-row-32 wording said `lockin.toml`; superseded by `decisions.md` row 32.) |
| ServiceContext | Per-call, cheap-to-clone handle (fittings) carrying `notify`, `cancelled`, `is_cancelled`, `request_id`; the same instance flows through middleware and the inner handler. |
| Session | Unit of conversation history, attached frontends, lock ownership, and `user_grants`; persisted under `${PROJECT_ROOT}/.rafaello/state/`. |
| Sink | A tool whose invocation is irreversible or has external effect (network, vcs_push, mail, workspace_write); declared in the manifest, snapshotted into `bindings.tool_meta.<n>.sinks`. |
| Sink confirmation | Core-enforced gate: a tool_request whose target declares any sink class is held until a matching `core.session.confirm_reply` arrives, unless a `user_grants` entry covers the invocation. |
| Stream RFC | One of the five subsystem design documents under `plans/streams/`; inputs to `overview.md`. |
| Stub (routing) | A registry entry for an installed-but-unspawned plugin's surface (RPC method, renderer kind, subscribe pattern); first access spawns the plugin and holds the dispatch until handshake. |
| Subprocess plugin | The v1 plugin runtime: a child process under lockin, speaking JSON-RPC on `RFL_BUS_FD`. |
| Taint | Structured `[{source, detail}, …]` provenance attached to bus events on `core.session.tool_result` and `core.session.tool_request`; populated by core, never trusted from plugins. Value-driven matching is per-router via `TaintMatchMap` (literal-hash + substring-containment arms, authoritative at `crates/rafaello-core/src/reemit/taint_match.rs`); ancestry inheritance is per-router via `ReferencedTaintIndex.lookup_request` / `lookup_result` (authoritative at `crates/rafaello-core/src/reemit/referenced_taint_index.rs`). m5b (`decisions.md` rows 50, 52, 57). |
| Topic | Dot-separated lowercase-segment string (`segment := [a-z0-9_-]+`); discriminators (tool name, correlation id) live in the payload, never in the topic. |
| Tool dispatch | The path from provider → core (taint + sink gate) → bound tool plugin → core (`in_reply_to` + taint validation) → subscribers; the only route from LLM output to tool execution. |
| Trifecta refusal | Compile-time refusal of a plugin grant that has all three of `reads_untrusted`, `has_outbound`, `has_workspace_write`; one-hop direct graph check; loud `--i-know-what-im-doing` override. |
| User grant (session) | An in-memory `user_grants` entry authorising a specific sink invocation for the rest of the session; created by `/grant` slash command, `always_allow_session` answer, or provider-extracted-then-confirmed proposal. Never written to the lock. |
