# m5a scope.md round-4 pi review

> Verdict: blocking
>
> Counts: B/1 M/6 N/4

Round 4 closes the main round-3 failure mode: the held confirmation is no longer consumed by re-emit before the gate can dispatch it, `confirm_reply.answer` is back to Stream A's two-value enum, the slash suffix spelling is live-source accurate, and the `parameters_schema` manifest-field detour is withdrawn.

One blocker remains. The new `always_allow_session` rewrite path says re-emit creates a `UserGrant`, but the specified live wiring gives re-emit neither a way to read the held `(plugin, tool, args)` context nor the `UserGrants`/audit handles needed to mutate/audit it. Several majors are precision issues in the new round-4 surfaces, mostly live-API drift around `ToolSchemaCatalog` and the exact `allow_secrets` implementation path.

## Findings

## Blockers

### B-1. `always_allow_session` grant creation is not mechanically wired

**Anchor:** §CT5 step 5, §CG1a, §CHAT1; live `reemit/mod.rs::ReemitRouter` constructor.

Round 4 correctly keeps `core.session.confirm_reply.payload.answer` to `"allow" | "deny"`, but the replacement path is not constructible as specified. §CT5 says re-emit, on frontend `always_allow_session`, reads the held entry to learn `(plugin, tool, args)`, calls `UserGrants::add(...)`, and audits `grant_added`. The surrounding wiring only says `ReemitRouter` gains `Arc<ConfirmState>`; live `ReemitRouter::new` currently has only `(broker, acl, active_provider, shutdown_rx)`, and §CG1a's `ConfirmState` methods are `reserve`, `is_held`, `try_resolve`, `try_take_for_timeout`, and `prior_outcome`. None returns the active `HeldConfirmation` or a reduced grant context without consuming it. Re-emit also is not given the `Arc<RwLock<UserGrants>>` or `AuditWriter` that CT5 step 5 needs.

So the core m5a populator "the user answering `always_allow_session`" remains broken even though the reply schema drift is fixed.

**Smallest acceptable fix:** Add one explicit path. Best fit: keep held-entry consumption in the gate, but let re-emit atomically mark the active confirmation as `session_grant_requested` after validating the frontend answer. Then `try_resolve` returns `(HeldConfirmation, session_grant_requested)` and CG4 creates the grant before dispatching. Alternative: add `ConfirmState::peek_grant_context(confirm_id)` plus pass `UserGrants` and `AuditWriter` into `ReemitRouter`. Either way, name the constructor fields and tests.

## Major

### M-1. `CorePluginService` provider detection cites a non-existent supervisor plan store

**Anchor:** §OP2 step 3; live `supervisor.rs:163`, `:813-826`, live `bus.rs:243-245`.

The `ToolSchemaCatalog` data-source fix is a good direction, but §OP2 says `is_provider(&canonical)` reads "the supervisor's existing per-plan record" and that the supervisor already stores compiled plans in `managed`. Live `managed` stores `ManagedSpawn` records, not compiled plans; `build_connection_service` currently receives only `canonical`.

This is mechanically fixable because live `Broker` already exposes `plugin_acl(&canonical) -> Option<PluginAcl>`, whose `provider_id` can answer the question.

**Smallest fix:** Specify `is_provider` as `self.broker.plugin_acl(&canonical).and_then(|a| a.provider_id).is_some()` or pass provider-ness into `build_connection_service` from `spawn(plan, ...)`. Do not cite `managed` as a compiled-plan store.

### M-2. OpenRPC-derived tool schemas still overclaim live validation and fixture coverage

**Anchor:** §OP2 step 7, §TP; live `manifest/validate_with_package.rs:25-38`.

Deriving model-facing schemas from `openrpc.json` is better than adding a new manifest field, but two details are still off:

1. §OP2 says m1's `validate_with_package` already enforces method-vs-tool consistency. Live source only checks `openrpc.json` sibling presence, entry resolution, `grant_match` path resolution, and exec-path syntax; it does not parse OpenRPC or compare methods to `provides.tools`.
2. The round-4 status says the mailcat fixture gets a tiny `openrpc.json`, but §TP still only specifies the crate behaviour and `grant_match` schema; it does not show or require the `send-mail` OpenRPC method that `ToolSchemaCatalog::build` needs.

**Smallest fix:** Move method-vs-tool consistency into the m5a `ToolSchemaCatalog` validation (or explicitly add it to `validate_with_package`) and add a TP bullet/snippet for `crates/rafaello-mailcat/openrpc.json` with the `send-mail` params.

### M-3. `env.allow_secrets` is wired through the wrong effective-grant function

**Anchor:** §OP6 effective-grant merge; live `compile.rs:248-312`, `sinks.rs:39-69`.

§OP6 says live `sinks::union_bundle` currently unions `pass` and merges `set`, and m5a extends it to union `allow_secrets`. Live `sinks::union_bundle` only handles filesystem/network for sink inference. The env merge used by the scrubber is `compile.rs::effective_grant`, which separately unions `env.pass` and merges `env.set`.

If implementation follows §OP6 literally, `allow_secrets` will not reach `compile_plugin`'s scrubber call.

**Smallest fix:** State that `compile.rs::EffectiveGrant` gains `GrantEnv.allow_secrets`, `compile.rs::effective_grant` unions/dedups it alongside `env_pass`, and only sink inference's `sinks::effective_grant` remains unchanged unless it has a separate need.

### M-4. `allow_secrets` validation/warning semantics are contradictory and not matched to live APIs

**Anchor:** status B-3 bullet, §OP6 validation rules, §OP4/§OP5 snippets; live `validate::lock` returns `Result<(), ValidationError>`.

Round-4 frontmatter says every `allow_secrets` name "must" appear in the same bundle's `env.pass`; §OP6 body says unused entries are warnings and install proceeds. The bundled manifest snippet intentionally has `pass = []` and three `allow_secrets`, so the status-rule version would reject the bundled manifest. Also, §OP6 says a reserved name in `allow_secrets` is rejected by existing `reject_reserved`, but live `reject_reserved` checks only `env.pass` and `env.set`, not `allow_secrets`. Finally, `validate::lock` has no warning return channel; a test named `validate_lock_warns_on_unused_allow_secrets_entry` is not implementable against the current signature without adding a diagnostics API or moving the warning to `rfl install`.

**Smallest fix:** Pick one semantic: either unused `allow_secrets` is accepted silently/with an installer-side diagnostic, or it is an error and the bundled snippets must only list the selected pass name. Add explicit validation for `allow_secrets` reserved names rather than relying on `reject_reserved`, or drop that rejection if unused names are harmless.

### M-5. CG4's `confirm_allowed_with_session_grant` classification is left as an unresolved design choice

**Anchor:** §CG4 allow arm.

The allow arm says the gate distinguishes ordinary allow from `always_allow_session` by either reading a fresh `UserGrants` entry "added in the same millisecond" or by re-emit adding `details.session_grant_added: true` to the `confirm_reply` payload, with "Final choice for `commits.md`." The first option is racey and not a protocol; the second adds a field to the Stream A `confirm_reply` payload shape immediately after B-2 fixed that drift.

**Smallest fix:** Carry this as internal state, not as an ad-hoc bus payload extension. For example, the B-1 fix can make `try_resolve` return a `session_grant_requested` flag, or CG4 can audit `confirm_allowed` only while the grant creation path audits `grant_added`.

### M-6. New audit kinds are used but not added to the audit schema list

**Anchor:** §CT5, §CG4, §AL1.

Round 4 introduces `confirm_malformed` and `confirm_resolved_after_timeout`; §CT5/§CG4 say they are audit-logged. §AL1's `kind` list still omits both. That will lead either to undocumented strings in `audit_events.kind` or missing tests.

**Smallest fix:** Add both to §AL1 (or drop `confirm_resolved_after_timeout` if it should be a debug log only) and add matching audit tests.

## Nits

### N-1. CT0 still references old `ConfirmState` method names

**Anchor:** §CT0 table/implications.

The duplicate row and implications still say `ConfirmState::resolve` / `mark_timed_out` / `is_held finds resolved`. Round 4 renamed the methods to `try_resolve` and `try_take_for_timeout`; align CT0 to avoid stale method copy/paste.

### N-2. OP6 contains an editorial "actually wait" aside

**Anchor:** §OP6 `rfl status` surface.

Remove the parenthetical "*(actually wait — ...)*" and leave only the corrected `GrantEnv.allow_secrets` wording.

### N-3. A11 still names the withdrawn lock path

**Anchor:** §A11.

A11 still says the lock side carries `bindings.capability.env.allow_secrets`. Round 4 moved the field to `grant.bundles.<bundle>.env.allow_secrets`; update the owner-judgment section.

### N-4. OP2 numbering repeats `7`

**Anchor:** §OP2.

After the tests list numbered item 8, the dead-code-allow paragraph is still numbered 7. Renumber to 9.

## Round-3 verification table

| Round-3 finding | Round-4 disposition | Verification |
|---|---|---|
| B-1 `ConfirmState::take_for_publish` consumes before gate | Mostly resolved | Re-emit no longer consumes the held entry; gate consumes via `try_resolve`. New B-1 is limited to the `always_allow_session` grant-creation side path. |
| B-2 `confirm_reply.answer = always_allow_session` schema drift | Mostly resolved | `confirm_reply.answer` is now `allow | deny`; re-emit rewrites `always_allow_session` to `allow`. New B-1/M-5 cover missing internal wiring, not the public answer enum. |
| B-3 `env.allow_secrets` not wired to live lock scrubber | Partially resolved → M-3/M-4 | Manifest and lock `GrantEnv` fields plus scrubber signature are now named. Remaining issues are the wrong effective-grant function and contradictory validation/warning details. |
| M-1 `CorePluginService` lacks schema data source | Partially resolved → M-1 | `ToolSchemaCatalog` supplies the catalog, but provider detection cites a non-existent compiled-plan store. |
| M-2 `parameters_schema` half-introduced | Mostly resolved → M-2 | The manifest field is withdrawn in favor of `openrpc.json`; live validation and fixture details still need correction. |
| M-3 malformed-answer re-hold timeout race | Resolved | Answer enum is validated before consulting/mutating `ConfirmState`; `re_hold` removed. |
| M-4 stale override risk | Resolved | Risk 16 now describes `allow_secrets` owner judgment, not `i_know_what_im_doing` as the selected bundled path. |
| M-5 dot-prefixed suffix entries | Resolved | §CT2/§SL0 now name un-dot-prefixed strings matching live `bus.rs`. |
| N-1 superseded M1.1 text | Resolved | Old reserve-`RFL_OPENAI_*` paragraph deleted, with only a non-instructional note left. |
| N-2 internal split reserved-env wording | Resolved | Internal split item 1 now names `allow_secrets` schema/scrubber work. |
| N-3 TUI1 payload/envelope confusion | Resolved | TUI1 now shows payload `{request_id, answer}` and envelope `in_reply_to`. |
| N-4 OP7 post-handshake test name | Resolved | Test renamed to `openai_tools_list_failure_exits_nonzero_and_supervisor_reports_crash.rs`. |

## Convergence call

Blocking count: **1**. Major count: **6**. Nit count: **4**.

Not ready for owner ratification yet: `always_allow_session` is a roadmap/demo-bar behaviour, and the current scope does not mechanically wire the grant creation path. I expect one more round if the next draft keeps the fix narrow: add an internal `session_grant_requested`/peek path, pass the required handles explicitly, and clean up the `allow_secrets`/ToolSchemaCatalog precision issues.

Owner-judgment items still worth surfacing:

1. m5a/m5b split: m5 is not closed until m5b ships the verbatim-exfil negative.
2. `grant_match` JSON-Schema-as-template-shape-contract: `/grant`-time validation, runtime structural-subset matching.
3. `env.allow_secrets`: additive manifest/lock schema and scrubber-signature change; owner should ratify after the mechanical details above are folded.
