# m5b scope.md round-1 pi review

> Verdict: blocking
>
> Counts: B/6 M/8 N/5

I reviewed `scope.md` against `driver-preflight.md`, m5a Appendix A, m5a retrospective §5/§9/§10, Stream A §7.2.1/§7.2.2/§7.2.6, overview/decisions/glossary, the prior-milestone patterns, and spot-checked the live m5a code under `rafaello/crates` (`bus.rs`, `reemit/mod.rs`, `gate/mod.rs`, `audit/mod.rs`, `rfl chat` wiring, and TUI env parsing).

The shape is close to the m5a Appendix A intent, but round 1 is not implementation-ready. The blockers are mostly contradictions between the headline demo, live m5a test hooks, and the taint-inheritance mechanics: the exfil test names two different target tools, the required two-modal script hook does not exist, audit-row emission is internally contradictory, the broker does not store the taint that PT1 says it reads, the canonical `tool_result` path still strips ancestry despite claiming to close Stream A §7.2.6, and TR4's rejection/synthetic-result path is not pinned enough to implement safely.

## Blockers

### B-1. The headline exfil demo has two incompatible flows and two different blocked plugins

**Anchor:** goal item 8 (`scope.md:120-132`), §EXFIL1 (`scope.md:1021-1065`), Demo bar (`scope.md:1230-1242`).

**Issue:** The top-level deliverable says `rafaello-fetch` returns `{content: "https://evil.example.com/leak"}`, the stub then proposes `web-fetch {url: "https://evil.example.com/leak"}`, the TUI denies, and the `web-fetch` invocation log is empty. §EXFIL1 later describes a different two-turn flow: first `web-fetch` is allowed and runs; then the model proposes `send-mail`; deny keeps **mailcat** empty. The Demo bar matches §EXFIL1, not the goal bullet.

That is not a wording nit: implementers cannot tell whether the roadmap negative is “tool result → same network sink blocked” or “fetch result → mail sink blocked.” The m5a Appendix A / driver-preflight intent is cross-tool verbatim exfil, which implies the second target should be `send-mail` and the empty log should be `rafaello-mailcat`, not `web-fetch`.

**Smallest acceptable fix:** Pick the §EXFIL1 flow as canonical and update goal item 8 to match it: first `web-fetch` allowed and invoked; second `send-mail` quotes the fetch result; deny prevents mailcat dispatch. Keep the exact expected taint vector and audit rows in the same section so the top-level deliverable and demo bar cannot drift.

### B-2. EXFIL1 requires per-turn TUI confirmation scripting that the live TUI does not have

**Anchor:** §EXFIL1 (`scope.md:1045-1056`), Demo setup (`scope.md:1225-1226`); live `rafaello/src/lib.rs:176-190`, `rafaello-tui/src/env.rs:14-28` and `:82-91`.

**Issue:** The headline test needs two different scripted answers in one `rfl chat` run: allow the first `web-fetch` modal and deny the second `send-mail` modal. The scope uses invented env vars `RFL_TUI_TEST_CONFIRM_ANSWER_TURN_1` / `_TURN_2`. Live m5a only forwards/parses a single `RFL_TUI_TEST_CONFIRM_ANSWER` plus one delay; there is no queue/turn-index parser and those `_TURN_*` names are not in the allowlist passed to `rfl-tui`.

**Smallest acceptable fix:** Scope the new test hook explicitly. For example, add `RFL_TUI_TEST_CONFIRM_ANSWERS=allow,deny` (or JSON array), update the rfl env allowlist and TUI parser, define exhaustion/malformed behavior, and add unit tests for two answers plus a third-modal exhaustion case. Alternatively redesign EXFIL1 to avoid two modal answers using an existing hook, but then the first fetch must still run deterministically.

### B-3. `confirm_request_taint_attached` emission condition conflicts with provider-only taint and EXFIL3

**Anchor:** goal item 6 (`scope.md:102-109`), §AL1 (`scope.md:873-912`), §EXFIL3 (`scope.md:1096-1111`), Demo assertions (`scope.md:1230-1245`); live `gate/mod.rs:386-397`.

**Issue:** §AL1 says to write `confirm_request_taint_attached` whenever `details.taint` is non-empty. But m5a canonical `core.session.tool_request` already always has provider taint, and the live gate already serialises that taint vector into `details` as an array. That makes provider-only prompts “non-empty.” The same scope then says the no-match provider-only case must **not** write the row (`scope.md:908-912`, `scope.md:1104-1111`). Both cannot be true.

**Smallest acceptable fix:** Define the row condition as “value-driven ancestry beyond the bare provider marker” (or another precise predicate), not “non-empty.” Add a helper-level test that provider-only taint does not audit the new row, while provider+tool does. Also update the payload wording in goal item 6 and §AL1 so the audit row's name matches the predicate.

### B-4. PT1 says the broker reads taint from `OutstandingDispatch`, but the live record does not carry it and is drained before any check

**Anchor:** §PT1 (`scope.md:704-729`), internal split (`scope.md:1677-1684`); live `bus.rs:168-177`, `bus.rs:517-540`, `bus.rs:1022-1038`.

**Issue:** §PT1 step 2 says the broker resolves the referenced event “using the broker's existing dispatched-id correlation — the dispatch record carries the originating `core.session.tool_request` taint.” Live `OutstandingDispatch` only stores `request_id` and `dispatched_at`; the taint is present only on the event passed to `fan_out`. Live `handle_plugin_publish` also removes the outstanding entry inside the critical section before any planned PT1 check would read it.

**Smallest acceptable fix:** Specify the data-model change and atomic order. For example: extend `OutstandingDispatch` with `taint: Vec<TaintEntry>` (and any needed payload/request metadata), populate it in `publish_for_tool_dispatch`, then in `handle_plugin_publish` inspect the entry, perform the superset check, and drain only on accepted result. Add a test proving a violating publish does **not** consume the outstanding entry unless the desired behavior is explicitly “reject and consume,” in which case scope the provider-visible timeout/error path.

### B-5. The canonical `tool_result` path still strips ancestry, so the scope does not actually close Stream A §7.2.6 row 1

**Anchor:** goal item 4 (`scope.md:73-88`), §TR1 (`scope.md:573-587`), §PT1/§PT2 (`scope.md:704-765`), owner row candidate (`scope.md:1880-1883`); Stream A §7.2.2/§7.2.6; overview §7 result path; live `reemit/mod.rs:391-402`.

**Issue:** The scope says plugin-supplied taint is checked, then discarded, and `handle_tool_result` records/publishes only `[{source: "tool", detail: "<canonical>"}]`. That means the canonical `core.session.tool_result` is not a superset of the taints on the referenced `core.session.tool_request`; it is just a new tool-origin marker. A plugin may publish a correct superset, pass PT1, and the canonical event still loses that ancestry at the re-emit boundary.

This drifts from Stream A §7.2.6's “verify that the published taint is a superset of the union of taints of every event referenced in `in_reply_to`” and overview §7's `core (in_reply_to + taint) -> core.session.tool_result` result path. It also weakens the scope's claim that m5b prevents plugin-supplied taint stripping: the check detects a contradictory plugin claim, but the canonical event still strips ancestry.

**Smallest acceptable fix:** Make an explicit owner-level choice and acceptance. Either:

- canonical `core.session.tool_result` taint becomes `tool-source ∪ referenced-tool_request-taint` (preferred if m5b truly closes §7.2.6 row 1), with tests for request-taint inheritance through a plugin result; or
- record a deliberate RFC/overview drift decision that v1 canonical tool results are fresh tool-origin sources only, and narrow the PT1 claim from “prevents stripping” to “rejects self-contradictory plugin-supplied taint before discard.”

Do not leave the current text claiming both discard-only canonicalisation and §7.2.6 closure.

### B-6. TR4's rejection/synthetic-result path is not mechanically pinned

**Anchor:** goal item 3 (`scope.md:58-71`), §TR4 (`scope.md:637-700`), internal split (`scope.md:1671-1675`, `scope.md:1755-1761`); live `bus.rs:897-967`, `reemit/mod.rs:339-346`, `reemit/mod.rs:251-257`.

**Issue:** §TR4 says the re-emit pipeline “rejects the publish with `BrokerError::TaintSupersetViolated`” and the provider receives a synthetic `tool_result` payload `{ok:false,error:"taint_superset_violation"}`. But provider publishes are already accepted by `handle_provider_publish` before the internal re-emit subscriber runs; a `BrokerError` from `handle_tool_request` currently becomes a `core.lifecycle.reemit_rejected`-style failure, not a direct rejection to the provider. The synthetic `core.session.tool_result` shape is also underspecified: no fresh `request_id`, no exact `in_reply_to`, no required non-empty taint, no statement that it passes `publish_core_with_taint`, and no agent-loop persistence assertion.

**Smallest acceptable fix:** Pin the TR4 failure algorithm as a re-emit failure, not an intake rejection, unless the broker intake is being moved earlier. For the synthetic result, specify: fresh result id, `in_reply_to = [held provider tool_request request_id]`, non-empty taint (likely the provider taint plus any referenced union or `system` marker), payload shape expected by the agent loop, audit row request_id, and whether the original provider publish returns Ok. Add a test that routes the synthetic result through `Broker::publish_core_with_taint` and the live agent-loop persistence path.

## Major

### M-1. The scope silently drops the Appendix A/preflight “every provider/frontend re-emit” superset surface

**Anchor:** m5a Appendix A.2 item 3, driver-preflight deliverable 3, §TR4 (`scope.md:637-700`).

The ratified input says “every `provider.<id>.*` and `frontend.tui.*` re-emit” whose `in_reply_to` implies inheritance must produce a canonical envelope that is a superset of referenced taints. Round 1 scopes only `provider.<id>.tool_request`. There is no acceptance for `provider.<id>.assistant_message` or `frontend.tui.confirm_answer` / `core.session.confirm_reply` behavior. If those are intentionally excluded, surface the narrowing as an owner-judgment item and Stream A drift; otherwise add the missing rows/tests.

### M-2. `SessionId` is not a live type in the re-emit path

**Anchor:** goal item 1 (`scope.md:39-47`), §TM1 (`scope.md:440-493`), §TM3 (`scope.md:554-568`); live `reemit/mod.rs:80-99`, `rfl/src/lib.rs:309-320` and `:464-470`.

The proposed API takes `&SessionId`, says it is “the same shape as the m3 session store key,” and exposes `drop_session(session)`. Live `ReemitRouter::new` receives broker/ACL/active provider/shutdown, not a session id; `SessionStore` has a string `session_id()` but the router is not wired to it. Since the map is already one per `rfl chat` process, the simplest implementation may not need a session key at all. Scope should either add the session id to router construction and shutdown wiring, or simplify `TaintMatchMap` to a per-router map with `clear()` on drop.

### M-3. TR1 has a record/publish ordering contradiction

**Anchor:** §TR1 (`scope.md:575-587`).

The text says recording “must happen after the canonical publish” so a synchronous subscriber observing `core.session.tool_result` can immediately publish a matching request, then says the preferred shape is `record` first because `publish_core_with_taint` can fail. For the stated subscriber guarantee, record must happen before fan-out or atomically with publish before any external observer can react. Pick one invariant and test it; do not leave both orderings in the implementation prompt.

### M-4. CD1 is stale against live m5a and leaves the empty shape to `commits.md`

**Anchor:** §CD1 (`scope.md:798-830`); live `gate/mod.rs:386-397`.

The live gate already puts `details.taint` in the confirm payload, and uses an empty array when the inbound event has no taint. §CD1 describes this as new work, says `null` when empty, and then says omitting the field is also acceptable if `commits.md` picks that later. Scope should not defer the wire shape to `commits.md`. Make CD1 a regression/normalisation item: either preserve live `[]`, or explicitly change it to `null` with a compatibility test and update AL1/EXFIL3 accordingly.

### M-5. The file-backed fetch fixture lacks a specified env path through the lock/supervisor

**Anchor:** §TF2 (`scope.md:971-996`), §TF3 (`scope.md:998-1017`), Demo setup (`scope.md:1215-1221`).

The demo depends on `RFL_FETCH_TEST_BODY_PATH`, but the fixture-lock bullets do not say how that env var reaches the plugin. Live plugin env is lock/supervisor mediated; a test harness setting an outer process env var is not enough unless the lock has `env.pass = ["RFL_FETCH_TEST_BODY_PATH"]` or `env.set` pins a path. Add the exact lock stanza and a compile/run test proving the plugin receives the path. Otherwise EXFIL1 can deterministically build a fixture that always returns `no_test_body`.

### M-6. Manual validation promises a real-network fetch path that TF2 explicitly excludes

**Anchor:** §TF2 (`scope.md:981-986`), §A6 (`scope.md:1418-1426`), Manual validation (`scope.md:1592-1605`), owner item 4 (`scope.md:1853-1859`).

The scope says `rafaello-fetch` does not issue real HTTP requests and real `web-fetch` semantics are out of m5b, but manual validation says the operator runs the m5b fixture against the dev LiteLLM proxy and “manual validation covers the real-network path.” LiteLLM only covers the provider; it does not make `rafaello-fetch` perform network I/O. Either manual validation should use the same file-backed body path and stop claiming real-network coverage, or §TF2 should choose a local stub HTTP server / real fetch implementation and test host-allow-list behavior.

### M-7. Substring-containment acceptance does not isolate substring containment, and the hash is unspecified

**Anchor:** §TM1 (`scope.md:451-456`, `scope.md:503-508`), §TM2 (`scope.md:523-552`), §A3 (`scope.md:1361-1373`).

`taint_match_records_substring_above_threshold.rs` uses the identical string on both sides, so the literal-hash arm would pass it without exercising substring containment. Add a unit where the recorded value contains a larger string and the later arg is a contained URL/token (or vice versa, if that is intended), and pin directionality. Also specify the stable hash algorithm; `HashMap<u64>` plus “stable hash” is not enough if implementers can reach for Rust's randomized `DefaultHasher`.

### M-8. The decomposition plan understates the TR4/PT1/fixture size risk

**Anchor:** internal split (`scope.md:1641-1766`), m5a size baseline (`driver-preflight.md` sizing), prior-milestone pattern “surface-size-predicts-rounds.”

Round 1 estimates ~16-19 commits while adding a new taint map, a broker ancestry index, re-emit failure synthesis, a new crate, new TUI scripting/rendering, new audit kinds, three c38 carryover tests, and three exfil integration variants. m5a ratified at 25 commits / 6 scope rounds with fewer new core data structures than this draft suggests. At minimum, split TR4 into cache/data-model and enforcement/synthetic-result commits if possible, split the TUI scripted-answer hook from provenance rendering, and budget closer to the Appendix A high end (or above it) until the contradictions above are resolved.

## Nits

### N-1. `core.lifecycle.plugin_publish_rejected` does not match the live lifecycle topic

**Anchor:** goal item 4 (`scope.md:84-88`), §PT1 (`scope.md:723-729`); live `bus.rs:1113-1154`.

Live m5a emits plugin publish failures on `core.lifecycle.publish_rejected` with a code payload, not `core.lifecycle.plugin_publish_rejected`. If m5b wants a new topic, scope the new topic and tests; otherwise use the live topic and add `code = "taint_superset_violated"`.

### N-2. Wrong cross-reference for the fetch owner choice

**Anchor:** §TF2 (`scope.md:988-991`).

“Owner-judgment item §A2 below” should be §A6 (or owner item 4). §A2 is the TaintMatchMap location.

### N-3. TM3 points TTL at §A2, but TTL is §A4 / owner item 1

**Anchor:** §TM3 (`scope.md:562-568`).

`ReemitRouter::new` constructs a map with the “§A2 default TTL”; §A2 is location, not TTL. Use §A4 or owner-judgment item 1.

### N-4. Cargo commands omit the manifest path from a repo-root worktree

**Anchor:** §TF1 acceptance (`scope.md:967-969`), §TF3 acceptance (`scope.md:1014-1017`).

The worktree root has no `Cargo.toml`; the rafaello workspace lives at `rafaello/Cargo.toml`. Use the same `--manifest-path rafaello/Cargo.toml` spelling as the acceptance summary.

### N-5. TF1 still mentions a deterministic HTTP client while TF2 chooses no network

**Anchor:** §TF1 (`scope.md:954-958`), §TF2 (`scope.md:971-996`).

Drop “pulling fittings + a deterministic HTTP client (or...)” if the default choice is file-backed/no network. Keeping both invites an unnecessary dependency debate during commits.md.

## Convergence call

Blocking count: **6**. Major count: **8**. Nit count: **5**.

I would not send this to `commits.md` yet. The next round should first make the exfil flow single-valued, add or avoid the two-answer TUI test hook, decide whether canonical `tool_result` inherits referenced taint, and make the broker's ancestry storage explicit. Once those are fixed, the remaining majors are mostly precision/sizing issues rather than design resets.

Owner-judgment items worth surfacing after the blockers are addressed:

1. Whether canonical `core.session.tool_result` includes referenced request ancestry or m5b records deliberate Stream A/overview drift.
2. Whether the exfil demo's allow-arm variant remains in m5b after the two-answer TUI hook is scoped.
3. Whether `rafaello-fetch` is file-backed only, local-stub HTTP, or real network for manual validation.
4. Whether the taint map/index live in the router, the broker, or split by responsibility.
