# Architecture decision log

Append-only. Reversals add a new row that references the reversed
decision; do not delete or edit prior rows.

Status values:

- `proposed` — written into `overview.md` and pending owner ratification. The default state for newly-added rows during the design phase.
- `locked` — committed, irreversible without re-opening v1 scope. Reserved for decisions the owner has explicitly locked.
- `ratified` — agreed by owner + claude + pi after debate. Requires the owner's sign-off on convergence (`plans/README.md` §Workflow Phase 1).
- `deferred-to-vN` — not in v1; revisit at the named version.
- `reversed` — superseded by a later row (see Reverses column).

| # | Date | Decision | Rationale | Status | Reverses |
|---|------|----------|-----------|--------|----------|
| 1  | 2026-05-07 | No embedded scripting language in v1; declarative TOML+Markdown plus subprocess plugins. | `streams/c-scripting/rfc-scripting-decision.md` — every motivation collapses into declarative config, and the cases that aren't (custom tools, loop, renderers) are exactly where lockin isolation pays. Reversible later by adding Luau; shipping then removing would cost users their init files. | proposed | — |
| 2  | 2026-05-07 | Lockin is the v1 sandbox; capsa is v2/v3. | Lockin is shipping today; capsa is research. The manifest schema is identical for both, so the swap is mechanical. | proposed | — |
| 3  | 2026-05-07 | Bus = broker (in core) + transport (fittings). Two layers, not one. | Avoids cross-stream confusion and keeps fittings free of topic/ACL/taint concepts. `overview.md` §4.1. | proposed | — |
| 4  | 2026-05-07 | Single canonical topic grammar + four namespaces (`core.*`, `provider.<provider-id>.*`, `plugin.<topic-id>.*`, `frontend.<attach-id>.*`). | Prevents publish-authority confusion; pinned in `streams/a-security/rfc-security-model.md` §5.1–5.2. The three id types have distinct lifetimes (overview §4.3). | proposed | — |
| 5  | 2026-05-07 | Topic-id form is `id_<base32(sha256(canonical))[0..16]>` with collision rejection at install. | Replaces the un-collision-safe `_`-substitution from the round-1 draft (pi-A round-2 finding 7). | proposed | — |
| 6  | 2026-05-07 | Provider plugins publish on `provider.<provider-id>.*`; core re-emits canonical `core.*` events. | Keeps `core.*` core-only (pi-A round-1 finding 2); CaMeL fits as a normal provider plugin without special-casing. | proposed | — |
| 7  | 2026-05-07 | Mandatory taint on `core.session.tool_request` and `core.session.tool_result`; structured `{source, detail}`; populated by core, not plugins. | Closes the verbatim-exfil case at the bus, not at plugin boundary. Pi-A round-2 finding 6/8 resolved by §7.2 of the security RFC. | proposed | — |
| 8  | 2026-05-07 | Mandatory `in_reply_to` on tool_result, RPC reply, confirm_answer, provider tool_request, and provider assistant_message. | Removes the "omit the field to strip taint" escape (pi-A round-2 finding 6). | proposed | — |
| 9  | 2026-05-07 | Bus-level sink confirmation, core-mediated, fail-closed. **Any sink call without matching `user_grants` requires confirmation, taint-independent.** | Tainting is provenance, not authorisation; this is the conservative rule pinned in `overview.md` §6.2. Reverses an earlier "non-user taint AND sink" formulation that survives in security RFC §10 (to be retro-patched). | proposed | — |
| 10 | 2026-05-07 | User-only taint is provenance, not authorisation; `user_grants` is the only confirmation bypass. | Pasted secrets and API keys in user prompts must not authorise themselves to flow to network sinks (pi-A round-2 finding 5). | proposed | — |
| 11 | 2026-05-07 | Trifecta graph check is one-hop direct, not transitive. | Cross-tool flows are caught at the bus (decision 9), not by per-plugin trifecta; the per-plugin check is therefore deliberately conservative-but-shallow. Security RFC §7.1.1. | proposed | — |
| 12 | 2026-05-07 | Carve-outs by compile-time decomposition; loud `--allow-credential-paths` override. | Lockin lacks deny-subpath precedence; decomposition is sufficient for v1 without an upstream lockin feature. Security RFC §7.3. | proposed | — |
| 13 | 2026-05-07 | Bus authentication = inherited socketpair fd (`RFL_BUS_FD`); no UDS path, no token. | Forge-proof inside the sandbox; lockin-compatible (pi-A round-1 finding 1); identical primitive transfers to capsa vsock. | proposed | — |
| 14 | 2026-05-07 | Helper plugins are a v1 primitive (`bindings.helper_for`, `RFL_HELPER_FD`). | Unblocks clean CaMeL Q-LLM isolation; generalises to any sandboxed sub-task (pi-A round-2 finding 2). Security RFC §7.4.1. | proposed | — |
| 15 | 2026-05-07 | Frontends are first-class bus principals (`frontend.<attach-id>.*`); UDS+token attach for external; **frontends are trusted UI principals, not lockin-sandboxed**. | Frontends speak for the user (answer confirmations); confining them as plugins would either break the trust model or make the TUI useless. Security RFC §5.7; overview §3, §10. | proposed | — |
| 16 | 2026-05-07 | Per-plugin private state at `${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/` is automatic, no grant; excluded from `has_workspace_write`. | Otherwise every plugin trips the trifecta (pi-A round-2 finding 8). Security RFC §7.5. | proposed | — |
| 17 | 2026-05-07 | Capability scoped bundles accepted, flattened at compile time; per-method enforcement happens in core, not in lockin. | Lockin can't switch policies live; v2 may tighten when it can. Manifest RFC §11 #1 → resolved here. | proposed | — |
| 18 | 2026-05-07 | fittings v1: `Request.id: Option<JsonRpcId>`, `Response.id: JsonRpcId`; two channels (unbounded response, bounded notification) with drop-on-full. | Pi-B round-2 findings 1, 2, 3 resolved by `streams/b-fittings/rfc-fittings-notifications.md` §2a, §3a, §3b. | proposed | — |
| 19 | 2026-05-07 | RenderTree is purely semantic (no colour/layout); core downgrades unsupported nodes to `Unknown { fallback }` server-side. | Keeps frontends dumb; matches Stream E §6. | proposed | — |
| 20 | 2026-05-07 | Streaming entry topics live under `core.session.entry.*`. | Stream E's unprefixed `session.entry.*` violates the namespace ACL; core is the publisher. Overview §4.2 reconciliation. | proposed | — |
| 21 | 2026-05-07 | The default LiteLLM provider ships as a bundled `rfl-litellm` subprocess plugin, not as built-in core code. | Preserves "providers ship as plugins" goal and uniform trust model (pi review 1 finding 4). Overview §8.1. | proposed | — |
| 22 | 2026-05-07 | Connection-scoped `ServerHandle::notify` is promoted from fittings "Future work" to a v1 requirement. | Bus broker fan-out cannot be implemented from `ServiceContext::notify` alone (pi review 1 finding 1). Overview §15.6. | proposed | — |
| 23 | 2026-05-07 | Bus payload schemas are owned by Stream A; JSON-RPC envelopes are owned by Stream B. | Pi review 1 finding 6: prevents divergent schemas. Overview §15.2; security RFC §9 #3 retro-patched. | proposed | — |
| 24 | 2026-05-07 | Eager-plugin handshake failure is fail-closed in v1; only override is `rfl plugin start --skip-eager`; no manifest knob. | Pi review 1 finding 5.3: removes the speculative `load.eager_failure` knob. Manifest knob deferred to v2. | proposed | — |
| 25 | 2026-05-07 | Manifest filename canonical: `rafaello.toml`. | Harmonises Stream A (was `plugin.toml`) and Stream F (already `rafaello.toml`). Pi review 1 finding 5.2. | proposed | — |
