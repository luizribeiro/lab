# m5a scope.md round-3 pi review

> Verdict: blocking
>
> Counts: B/3 M/5 N/4

Round 3 fixes the two explicit round-2 blockers on their narrow facts: `RFL_OPENAI_ENDPOINT_URL` / `RFL_OPENAI_MODEL` are no longer reserved, and §CT0 restores Stream A's `payload.request_id == confirmation correlation id` semantics. The slash-command correlation table, install-validation ordering, and `jsonschema` dependency are also materially better.

However, the new round-3 surfaces introduce three handoff blockers. The `ConfirmState` design consumes the held entry in re-emit before the gate can dispatch it, so the happy path is mechanically broken. The confirm-reply payload now carries `always_allow_session`, which is another Stream A schema drift. And the new `env.allow_secrets` opt-in is specified through a nonexistent lock path and cannot affect the live lock-only scrubber unless the lock/env structures are extended explicitly.

## Findings

## Blockers

### B-1. `ConfirmState::take_for_publish` consumes the held entry before `ConfirmationGate` can use it

**Anchor:** §CT5 steps 3-6, §CG1a method table, §CG4. Carryover from round-2 M-5.

Round 3 names a shared `ConfirmState`, but the ownership flow is internally inconsistent. §CT5 says the re-emit arm calls `take_for_publish(payload.request_id)`, which replaces `Active(HeldConfirmation)` with `ResolvedByAnswer` and returns the `HeldConfirmation` to re-emit. Then re-emit publishes only:

```json
{ "request_id": "<correlation_id>", "answer": "<...>" }
```

on `core.session.confirm_reply`. §CG4 then says the gate receives that reply and looks up `held[confirm_key]` to dispatch/deny. At that point the shared map no longer contains an active held entry; it contains a tombstone. The `HeldConfirmation` returned to re-emit is not carried on the bus and is not otherwise handed to the gate. The normal allow/deny path therefore cannot recover the held tool_request to dispatch or synthesize deny.

This also contradicts the round-3 status text and CT0 table references to `ConfirmState::resolve`: no `resolve` method exists in the actual §CG1a method list.

**Smallest acceptable fix:** Pick one owner for consuming the held entry. The cleaner fit with the existing protocol: re-emit validates only (`in_reply_to == payload.request_id`, answer enum, `ConfirmState::is_held` / prior-outcome classification) and publishes `core.session.confirm_reply`; the gate's CG4 handler atomically `resolve`/`take`s the active held entry and dispatches/denies. Alternatively, make re-emit own final dispatch/deny and delete the gate's CG4 bus-consumer path, but do not split the `HeldConfirmation` across an uncarried bus payload.

### B-2. `core.session.confirm_reply.answer = "always_allow_session"` drifts from Stream A

**Anchor:** §CT5 steps 4/6, §CG4, Stream A §5.6.

Round 3 fixes the `request_id` meaning in the confirm payloads, but introduces/retains a second payload-schema drift. Stream A §5.6 defines:

```json
// frontend.<id>.confirm_answer
{ "request_id": "<uuid>", "answer": "allow" | "deny" | "always_allow_session" }
// core.session.confirm_reply
{ "request_id": "<uuid>", "answer": "allow" | "deny" }
```

The frontend answer may contain `always_allow_session`; the core reply may not. §CT5 accepts `always_allow_session` and §CG4 handles it after it arrives on `core.session.confirm_reply`, which changes the ratified core reply schema.

**Smallest acceptable fix:** Keep `core.session.confirm_reply.payload.answer` to `allow | deny`. If the frontend selected `always_allow_session`, core should create the session grant and emit/audit a canonical reply with `answer = "allow"` (plus audit/detail metadata if needed). If m5a intentionally wants to broaden `confirm_reply`, mark it as an explicit owner/decision-row schema change rather than saying Stream A field names are lifted verbatim.

### B-3. `env.allow_secrets` is not mechanically wired to the live lock-only scrubber

**Anchor:** §OP5, §OP6, §A11, §W3/§OP4 snippets; live `compile.rs:191-194`, `scrubber.rs:35-43`, `manifest/capabilities.rs`, `lock/grant.rs`.

The UX goal is good, but the specified data path cannot work as written:

- `compile_plugin` receives the **lock** and calls `scrubber::strip(&eff.env.pass, entry.flags.i_know_what_im_doing)`. It does not read the manifest.
- Live `GrantEnv` has only `pass` and `set`; live `EnvCapabilities` has only `pass` and `set`.
- §OP6 / §A11 say the manifest value is snapshotted into `bindings.capability.env.allow_secrets`; no such live lock path or `Bindings` field exists (and `bindings.capability` singular is not a live concept).
- The §OP5 lock TOML snippet still only has `pass` and `set`; it does not show any lock-carried `allow_secrets` value.
- The §OP4 bundled manifest snippet does not include the `[capabilities.default.env] allow_secrets = [...]` that the prose says the bundled manifest declares.

With the current specified lock shape, `LITELLM_API_KEY` still matches `SECRET_PATTERNS` and is stripped unless `flags.i_know_what_im_doing` is set; round 3 removed that fallback from the happy path.

**Smallest acceptable fix:** Define the actual live fields. For example: add `allow_secrets: Vec<String>` to manifest `EnvCapabilities`, lock `GrantEnv`, effective grant merging, and `EnvPlan`/status surfaces as needed; change `scrubber::strip` to accept the allow-list; show it in the lock TOML and bundled manifest snippets; add validation that every `allow_secrets` name is a plain env var and only affects names present in `env.pass`. Or explicitly abandon OP6 and restore the round-2 override-flag path.

## Major

### M-1. `CorePluginService` still lacks a concrete source for compiled tool schemas

**Anchor:** §OP2, §CHAT1; live `supervisor.rs:271-282`, `:813-826`, `broker_acl.rs:27-30`, `compile.rs:91-96`.

Round 3 correctly removes `BrokerAcl.fittings_methods` and `SpawnError::PostHandshakeFailure`, but the replacement still is not mechanically complete. Live `PluginSupervisor` stores a `Broker`, not a `BrokerAcl` or compiled-plugin map. Live `BrokerAcl` has `plugins`, `tool_routes`, and `frontends`; it does not contain `tool_meta`. The compiled `tool_meta` lives in `CompiledPlugin`, outside `BrokerAcl`.

§OP2 says `CorePluginService` captures `Arc<BrokerAcl>` and walks `BrokerAcl.tool_routes` plus `CompiledPlugin.tool_meta`, but does not say where `PluginSupervisor::build_connection_service` gets the compiled-plugin table or a schema cache. §CHAT1 also retains stale wording that `rfl chat` registers `core.tools_list` "on the broker's fittings server", contradicting §OP2's per-connection supervisor service design.

**Smallest fix:** Add an explicit construction path, e.g. `PluginSupervisor::new(broker, config, ToolSchemaCatalog)` or a new field containing `BTreeMap<CanonicalId, CompiledPlugin>`/precomputed `Vec<ToolSchema>`, and pass provider-ness into `build_connection_service` (or query `broker.plugin_acl`). Update §CHAT1 to remove the broker-server registration language.

### M-2. `parameters_schema` is introduced halfway and the sink fixture does not provide one

**Anchor:** §OP2 step 5, §TP1-TP2, acceptance drift list; live `manifest/provides.rs:33-40`, `lock/bindings.rs:29-39`, `compile.rs:91-96`.

`core.tools_list` needs OpenAI tool parameter schemas. Round 3 adds a new `parameters_schema: Option<SafePath>` concept in §OP2, but it is not carried through all the places a m1 manifest/lock extension needs:

- live manifest/lock/compiled `ToolMeta` currently have `sinks`, `grant_match`, and `always_confirm` only;
- §OP2 says the JSON is "loaded and embedded in `ToolMeta` at compile time", but does not name the new lock field or compiled field type;
- the `rafaello-mailcat` fixture in §TP declares `grant_match` but no `parameters_schema`, while §OP2's `ToolSchema` response requires `parameters_schema: serde_json::Value`.

**Smallest fix:** Either derive tool parameter schemas from the existing required `openrpc.json` sibling, or fully specify the new field in manifest, lock `bindings.tool_meta`, compiled `ToolMeta`, validation, and fixture TOML. Add a `send-mail` parameters schema fixture distinct from the grant matcher if the model-facing shape differs.

### M-3. Malformed confirm answers consume/re-hold the entry and race the timeout

**Anchor:** §CT5 steps 3-4, §CG1a `re_hold`, §CG5.

Even after B-1's ownership is fixed, the malformed-answer sequence is risky as specified. §CT5 takes the active held entry before validating the answer enum, then re-inserts it on malformed input. If a malformed answer arrives near the 60s deadline, the timeout task may observe `ResolvedByAnswer` and skip publishing the fail-closed synthetic deny, after which `re_hold` can resurrect an already-expired entry.

**Smallest fix:** Validate the answer string before consuming/resolving the held entry. Unknown/duplicate/late classification can still use `prior_outcome`, but malformed payloads should not mutate `ConfirmState` at all (or should check the deadline while re-holding and immediately time out if expired).

### M-4. Stale round-2 `i_know_what_im_doing` text remains in Risks

**Anchor:** Risk 16, §OP5/§OP6/manual validation.

Risk 16 still says the bundled `rfl-openai` lock entry requires `flags.i_know_what_im_doing` because `LITELLM_API_KEY` matches the scrubber, and proposes a bundled-plugin override display. That directly contradicts round 3's selected `env.allow_secrets` path and manual-validation text, which say no nuclear override flag is needed.

**Smallest fix:** Rewrite or delete Risk 16. The remaining risk is the new `allow_secrets` schema/UX owner-judgment item, not the red override flag.

### M-5. Dot-prefixed suffix-list wording is easy to implement incorrectly against live `bus.rs`

**Anchor:** §CT2 / §SL0; live `bus.rs:17-31`.

The scope repeatedly says to add `.confirm_request`, `.slash_command`, etc. to `REQUEST_ID_REQUIRED_SUFFIXES`. Live `bus.rs` stores the **last segment without a dot** (`"tool_request"`, `"tool_result"`, `"assistant_message"`, `"user_message"`) and compares it to `topic.rsplit('.').next()`. Adding literal dot-prefixed strings would silently make the enforcement not fire.

**Smallest fix:** State the exact entries as `"confirm_request"`, `"confirm_reply"`, `"confirm_answer"`, `"slash_command"`, and `"command_result"`, and reserve dot-prefixed spelling only for prose.

## Nits

### N-1. Superseded M1.1 text still says to reserve `RFL_OPENAI_*`

**Anchor:** §M1.1.

The paragraph is labelled superseded, but leaving the exact old "extend to nine" instruction in the implementation body invites copy/paste errors. Delete it or move it to the history block only.

### N-2. Internal split still says "m1 reserved-env extension"

**Anchor:** §Internal split item 1.

Round 3 changed M1.1 to "no new reserved env-var names". The commit grouping should say workspace/crate scaffolds + env scrubber tests / allow-secrets schema, not reserved-env extension.

### N-3. TUI1 shows `in_reply_to` inside the confirm-answer payload literal

**Anchor:** §TUI1.

`in_reply_to` is an envelope field per §CT0. Reword the example to show payload `{request_id, answer}` plus envelope `in_reply_to = [confirm_id]`.

### N-4. OP7 keeps a `post_handshake_failure` test name after removing that error category

**Anchor:** §OP7.

`openai_post_handshake_failure_propagates_through_supervisor.rs` still uses the removed framing. Rename to the selected behaviour, e.g. `openai_tools_list_failure_exits_nonzero_and_supervisor_reports_crash.rs`.

## Round-2 verification table

| Round-2 finding | Round-3 disposition | Verification |
|---|---|---|
| B-1 OpenAI endpoint/model reserved and set | Resolved | §M1.1 now says no new reserved names; §OP5 calls endpoint/model plugin-config env vars. New B-3 is about the added `allow_secrets` path, not endpoint/model reservation. |
| B-2 confirm payload `request_id` semantics | Narrowly resolved | §CT0 now uses payload `request_id` as the confirmation correlation id and allows envelope ids to differ on answer/reply. New B-2 flags a separate confirm-reply `answer` schema drift. |
| M-1 `core.tools_list` against nonexistent APIs | Partially resolved → M-1/M-2 | Phantom `BrokerAcl.fittings_methods` and `PostHandshakeFailure` are gone. The new `CorePluginService` still lacks a concrete compiled-schema data source, and `parameters_schema` is only half-specified. |
| M-2 slash-command correlation | Resolved | §SL0 removes payload ids, pins envelope `request_id`/`in_reply_to`, and §CT2 adds both suffixes. Only nit/precision remains on live suffix string spelling (M-5). |
| M-3 install validate/trifecta ordering | Resolved | §Tr1 now applies override flags before `validate::lock` and maps `ValidationError::TrifectaRefused`; no independent post-validation gate remains. |
| M-4 `jsonschema` dependency missing | Resolved | §W1 adds a workspace `jsonschema` dependency and risk note. |
| M-5 held-confirmation map ownership | Not resolved → B-1/M-3 | `ConfirmState` is named, but the consume/dispatch ownership is broken and malformed-answer re-hold races timeout. |
| M-6 OpenAI default model conflict | Resolved | §OP1 makes `RFL_OPENAI_MODEL` required; fixture/manual lock sets `vllm/qwen3.6-27b`; missing-model negative named. |
| N-1 status wording | Resolved | Status reflects pi-2 accepted the pi-1 M-6 pushback. |
| N-2 row 29 renderer wording | Resolved | Inputs now say TUI-internal overlay, not built-in renderer. |
| N-3 m4 retro stale-id reader | Resolved | Inputs now identify broker-owned `outstanding_dispatched` as stale-id reader. |
| N-4 OP5 stripping layer | Resolved | OP5 tests now inspect compiled plan / `compile_plugin` scrubber, not `validate::lock`. |
| N-5 Appendix A `Confirm` render kind | Resolved | Appendix A.3 names the TUI confirmation overlay and no `RenderNode::Confirm`. |

## Convergence call

Blocking count: **3**. Major count: **5**. Nit count: **4**.

I do not think this is ready for owner ratification yet. The remaining issues are concentrated and fixable, but two are core protocol mechanics (`ConfirmState` handoff and confirm-reply schema), and one is the new manifest/lock scrubber path.

Expected rounds remaining: **1-2** if round 4 keeps the design changes narrow: move held-entry consumption back to the gate, keep `confirm_reply` Stream-A-shaped, and make `allow_secrets` a concrete lock-carried env field or revert to the override path.

Owner-judgment items to keep surfacing:

1. m5a/m5b split remains owner-visible; m5 is not closed until m5b ships the verbatim-exfil negative.
2. `grant_match` JSON-Schema-as-template-shape-contract remains an owner-visible semantic choice.
3. `env.allow_secrets` is a new additive manifest/lock schema choice; owner should ratify it only after the mechanical lock/scrubber path is made precise.
