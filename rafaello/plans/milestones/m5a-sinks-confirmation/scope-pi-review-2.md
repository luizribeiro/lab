# m5a scope.md round-2 pi review

> Verdict: blocking
>
> Counts: B/2 M/6 N/5

Round 2 closes most of the round-1 structural problems. The slash-command process boundary, synthetic deny event shape, outstanding-dispatch map split, transient TUI overlay, and m5a/m5b acceptance framing are much improved.

Two issues still block handoff: the OpenAI env model reserves names that the same lock entry must set, and the new confirmation correlation table changes Stream A's payload `request_id` meaning for answers/replies. Several new round-2 surfaces also need tightening before `commits.md`: `core.tools_list` is described against non-existent live APIs, slash-command correlation is not pinned with the same rigor as confirmations, the bounded `rfl install --fixture` algorithm has an ordering bug around `validate::lock`, and the new `jsonschema` dependency is referenced but not declared.

## Findings

## Blockers

### B-1. `RFL_OPENAI_ENDPOINT_URL` / `RFL_OPENAI_MODEL` are both reserved and required in `env.set`

**Anchor:** §OP5 (`scope.md:1738-1753`), §M1.1 (`scope.md:1907-1923`), manual validation (`scope.md:2477-2485`). Carryover from round-1 B-5, partially resolved but with a new contradiction.

**Issue:** Round 2 withdraws the `env.pass` rename syntax, which is good. But it now says m5a adds `RFL_OPENAI_ENDPOINT_URL` and `RFL_OPENAI_MODEL` to `RESERVED_ENV_VARS` and rejects them “when present in `env.set` or `env.pass` of any plugin's lock entry” (§M1.1). The `rfl-openai` lock snippet in §OP5 and manual validation require exactly those two keys in `[...grant.bundles.default.env.set]`. Under the live compiler this fails: `compile.rs` calls `scrubber::reject_reserved(&eff.env.pass, &eff.env.set)?` before emitting `EnvPlan`, and `scrubber.rs` rejects any reserved key in `env.set`.

**Smallest acceptable fix:** Do not put plugin-consumed OpenAI config env vars in the core reserved list, or explicitly distinguish “core-injected reserved” from “plugin-config env names allowed only for the bundled openai plugin.” The simpler fix: keep `RFL_OPENAI_ENDPOINT_URL` / `RFL_OPENAI_MODEL` unreserved user-set env keys, add collision tests that they pass through in the `rfl-openai` lock, and only reserve actual core-injected names.

### B-2. CT0 changes Stream A's confirmation payload `request_id` semantics

**Anchor:** §CT0 table (`scope.md:933-973`), §CT5 (`scope.md:1018-1037`). Carryover from round-1 B-2, partially resolved.

**Issue:** The new CT0 table pins `frontend.tui.confirm_answer.payload.request_id` to the answer event id and `core.session.confirm_reply.payload.request_id` to the reply event id, with the confirmation id only in envelope `in_reply_to`. That conflicts with Stream A §5.6's payload schema, where `confirm_answer` and `confirm_reply` payload `request_id` is the confirmation request id being answered/replied to:

```json
// frontend.<id>.confirm_answer
{ "request_id": "<uuid>", "answer": "allow" | "deny" | "always_allow_session" }
// core.session.confirm_reply
{ "request_id": "<uuid>", "answer": "allow" | "deny" }
```

In that schema, `<uuid>` is the confirmation correlation id, not a fresh answer/reply event id. Round 2 therefore resolves the “two id spaces” problem by silently changing the ratified payload shape. A frontend or audit reader following Stream A would send/read `payload.request_id = confirm_id` and fail m5a's “payload equals envelope id” check for answers.

**Smallest acceptable fix:** Preserve Stream A payload semantics: `payload.request_id` on all three confirmation payloads is the confirmation id. The bus envelope `request_id` may still be fresh per event for `.confirm_answer` / `.confirm_reply` if m5a wants event ids, but CT0 must then state that payload and envelope ids differ on answer/reply and that `in_reply_to == [payload.request_id]`. Alternatively, if m5a intentionally changes the Stream A payload schema, add an explicit owner/decision-row item and mark the RFC drift; do not call the field names “lifted verbatim.”

## Major

### M-1. `core.tools_list` is specified against non-existent live API surfaces

**Anchor:** §OP2 (`scope.md:1635-1672`), §CHAT1 (`scope.md:1949-1955`), Risk 6 (`scope.md:2376-2387`).

Round 2 correctly removes the lossy `tools_advertised` startup event, but the replacement is underspecified against live source. The scope says `BrokerAcl.fittings_methods` “m2 already exists” and is extended with `core.tools_list`; there is no `fittings_methods` in live `BrokerAcl` (`broker_acl.rs` has `plugins`, `tool_routes`, `frontends`). It also says a failed tools-list call becomes `SpawnError::PostHandshakeFailure`; no such live variant exists. Live `PluginSupervisor` composes only `bus.publish` in production; its `ExtraServiceFactory` is `#[cfg(any(test, feature = "test-fixture"))]`.

**Smallest fix:** Specify the actual production implementation: e.g. add a production `CorePluginService` composed in `PluginSupervisor::build_connection_service` for provider connections, handling `core.tools_list` before falling through to method-not-found; add the concrete error variant if needed. Remove references to nonexistent `BrokerAcl.fittings_methods` / `PostHandshakeFailure`, or explicitly scope their addition with tests.

### M-2. Slash-command / command-result correlation is not pinned like CT0

**Anchor:** §SL2-3 (`scope.md:1441-1492`), §CT2 (`scope.md:983-993`).

The slash-command path repeats the round-1 correlation ambiguity in a smaller form. §SL2 says `frontend.tui.slash_command` has mandatory envelope `request_id`, and the payload also contains `request_id`. §SL3's `core.session.command_result` payload contains both `request_id` and `in_reply_to`, but does not state whether those are payload fields, bus envelope fields, or both. CT2 only extends the suffix list to `.confirm_*`; it does not actually add `.slash_command` / `.command_result` even though SL2 says slash_command is “added to the same suffix list.”

**Smallest fix:** Add an SL0 mini table mirroring CT0: envelope id, payload id (or remove payload id), envelope `in_reply_to`, stale behavior, and suffix-list changes for `slash_command` and `command_result`. Keep `in_reply_to` in the bus envelope, not duplicated inside payload, unless a new payload schema deliberately repeats it.

### M-3. `rfl install --fixture` runs `validate::lock` before the override/refusal logic is coherent

**Anchor:** §Tr1 (`scope.md:838-883`).

The algorithm says construct candidate, run `validate::lock`, then run `trifecta::evaluate`; if refused and `--i-know-what-im-doing` was not passed, return `InstallError::TrifectaRefused`; with override, set the flag before the “second” `evaluate`. But live `validate::lock` already calls `trifecta::evaluate` and returns `ValidationError::TrifectaRefused`. Therefore a trifecta candidate without the flag never reaches the explicit evaluate step, and a candidate with `--i-know-what-im-doing` must have the flag set before `validate::lock`, not before a second evaluate.

**Smallest fix:** Reorder and state it mechanically: build candidate; if override flags were passed, set them; run `validate::lock`; map `ValidationError::TrifectaRefused` to `InstallError::TrifectaRefused` for UX; optionally call `trifecta::evaluate` only to collect/print booleans before validation, but do not describe validation and explicit evaluate as independent gates.

### M-4. `jsonschema` is used but not declared in workspace/deps section

**Anchor:** §UG2 (`scope.md:1341-1383`), §W1 (`scope.md:780-789`), Risk 15 (`scope.md:2444-2449`).

Round 2 changes m5a from structural-only matching to JSON-Schema validation of grant templates at `/grant` time. That may be acceptable, but §W1 does not add `jsonschema` to `rafaello-core` or the workspace dependencies. Risk 15 mentions it, but the commit-shaped workspace section is the dependency contract for Phase 3.

**Smallest fix:** Add `jsonschema = { workspace = true }` to §W1 / `rafaello-core` dependency planning, list the exact crate version or workspace alias expectation, and include its macOS/CI risk in the acceptance notes.

### M-5. Re-emit/gate ownership of the held-confirmation map is still not mechanically specified

**Anchor:** §CT5 (`scope.md:1018-1037`), §CG1 (`scope.md:1041-1054`), §CG4 (`scope.md:1103-1123`).

CT5 says the re-emit pipeline validates `confirm_answer` by checking the gate's held-confirmation map; CG1 says the map is owned by `ConfirmationGate`. The live m4 `ReemitRouter` is a separate task/module and has no dependency on gate state. This can be implemented with a shared `Arc<ConfirmState>`, but the scope does not name that shared type or construction ordering. Without it, Phase 3 can easily split validation between reemit/gate and resurrect duplicate/late races.

**Smallest fix:** Introduce a named `ConfirmState` shared by `ConfirmationGate` and the re-emit confirm-answer arm. State which methods are atomic (`reserve`, `resolve`, `mark_timed_out`, `is_held`) and which component owns publishing/auditing for unknown/late/duplicate answers.

### M-6. OpenAI default model conflicts with the roadmap default

**Anchor:** §OP1 wire table (`scope.md:1619-1620`), §OP5 (`scope.md:1740-1742`).

The roadmap/pre-flight name default model `vllm/qwen3.6-27b`. OP5 sets that in the lock, but OP1 says if `RFL_OPENAI_MODEL` is unset the plugin defaults to `gpt-4o-mini`. That bakes an OpenAI-specific default into the generic `rfl-openai` plugin and conflicts with the m5 row's default. Since endpoint/model are deployment config, a missing model should probably be a typed config error (or default to the roadmap value only in the fixture lock), not silently switch to another provider's model.

**Smallest fix:** Make `RFL_OPENAI_MODEL` required for m5a fixture/manual validation, or default to `vllm/qwen3.6-27b` only through the lock materialisation. Add a negative `openai_missing_model_env_errors_before_request.rs` if required.

## Nits

### N-1. Status says “all 8 M resolved” while also saying M-6 is pushed back

**Anchor:** status block (`scope.md:3-4`, `scope.md:85-94`).

Reword to “7 resolved, M-6 pushed back and accepted/awaiting pi decision” after this review.

### N-2. Decisions row 29 input still calls the overlay a built-in renderer

**Anchor:** decisions input (`scope.md:582-584`).

Round 2 removed `RenderNode::Confirm`, but the input list still says “the `confirm_request` modal is a built-in renderer.” Reword to “TUI-internal overlay; no subprocess renderer.”

### N-3. m4 retro inheritance still says “gate's outstanding map” is the stale-id reader

**Anchor:** m4 retrospective inputs (`scope.md:695-699`).

Round 2 moved stale-correlation to the broker-owned `outstanding_dispatched` map. Update this bullet to avoid contradicting §OM.

### N-4. OP5 test description says `validate::lock` strips `env.pass`

**Anchor:** §OP5 tests (`scope.md:1775-1778`).

Live stripping happens in `compile_plugin` via `scrubber::strip`, not `validate::lock`. Adjust the test wording to “compiled plan strips the pass entry” or make validation reject if that is the intended new behavior.

### N-5. Appendix A still inherits deleted `Confirm` render kind / §RC

**Anchor:** Appendix A.3 (`scope.md:2770-2774`).

Remove “`Confirm` render kind (§RC)” and say m5b inherits the TUI confirmation overlay.

## Round-1 verification table

| Round-1 finding | Round-2 disposition | Verification |
|---|---|---|
| B-1 slash commands mutate core memory from TUI | Resolved | Slash commands are now `frontend.tui.slash_command`; core owns mutation and emits `core.session.command_result`. New M-2 only asks for tighter correlation wording. |
| B-2 confirmation correlation | Partially resolved → B-2 | CT0 exists and covers duplicates/late answers, but payload `request_id` semantics now conflict with Stream A §5.6. |
| B-3 synthetic deny invalid under m4 envelope | Resolved | §CG4a pins fresh `request_id`, `in_reply_to`, non-empty system taint, and content field; tests named. |
| B-4 `tools_advertised` startup race | Partially resolved → M-1 | Bus event race is gone; new `core.tools_list` RPC needs live-source-accurate service wiring. |
| B-5 OpenAI env conflicts | Partially resolved → B-1 | Rename syntax withdrawn and lock TOML shape fixed, but endpoint/model env vars are both reserved and set. |
| B-6 install/status underspecified | Partially resolved → M-3 | `--fixture` boundary is much clearer; validate/trifecta ordering still wrong. |
| B-7 outstanding map wrong owner/race | Resolved | §OM is broker-owned and atomically checked/drained in `handle_plugin_publish`; duplicate/race tests named. |
| M-1 split owner decision | Resolved | Acceptance explicitly says m5a is not full m5; m5b required before m5 closes. Owner ping still should surface split. |
| M-2 OpenAI wire/error behavior | Mostly resolved → M-6 | Wire table and negatives added; only model-default conflict remains. |
| M-3 multi-pending confirmations | Resolved | §CG7 / §TUI3 specify queueing, short-circuit, stale answers, per-held timeouts, and tests. |
| M-4 modal render kind ambiguity | Resolved | `RenderNode::Confirm` / §RC withdrawn; overlay is TUI-internal and transient. |
| M-5 `user_grants` matcher contradiction | Resolved with owner judgment | Schema-as-template-shape-contract is explicit, lock-pinned, and tested. Ensure `jsonschema` dep is added (M-4). |
| M-6 dead-code removal acceptance | Pushed back, accepted | I accept the pushback. Dropping the acceptance bullet while keeping the m4 retro follow-up open is the right outcome. |
| M-7 negative test gaps | Mostly resolved | Confirm, slash, grant-race, duplicate/late tests added. M-2 asks for slash correlation table so those tests are unambiguous. |
| M-8 sink enum churn | Resolved | Storage remains `Vec<String>`; typed enum is parser/accessor only. |
| N-1 bad §OA reference | Resolved | Now references §OP2 / `core.tools_list`. |
| N-2 `fittings` dependency name | Resolved | Live fittings crate names listed. |
| N-3 `rfl install` bin target wording | Resolved | Now existing `rfl` subcommand. |
| N-4 stray slash | Resolved | Removed. |
| N-5 reserved-env count | Resolved | Count corrected to seven→nine, though B-1 disputes what should be reserved. |
| N-6 env var unset/set wording | Resolved | Negative 2 now says no pre-existing grant; automated deny after 10ms. |

## Convergence call

Blocking count: **2**. Major count: **6**. Nit count: **5**.

This is much closer than round 1. If the env-reservation contradiction and confirmation payload-schema drift are fixed cleanly, I expect convergence in 1-2 more rounds. The remaining majors are mostly wording/API-precision issues introduced by round-2's better design choices.

Owner-judgment items still worth surfacing:

1. The m5a/m5b split remains an owner-visible commitment: m5 is not closed until m5b ships the verbatim-exfil negative.
2. The `grant_match` “schema validates matcher template, runtime structural subset” interpretation is now explicit; owner should be aware it is not full JSON-Schema matching per tool call.
3. The bundled `rfl-openai` API-key path currently wants `flags.i_know_what_im_doing` because of `*_KEY` scrubbing; owner should weigh the UX of a scary override on the default provider before Phase 3.
