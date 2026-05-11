# m4 — provider fixture + secure agent loop + one read-only tool — retrospective

> **Status: round-1 draft, 2026-05-11.** Awaiting adversarial pi
> review per `plans/README.md` §"Patterns from prior milestones"
> ("Retrospective drafts deserve the same adversarial review as
> scope and commits"). m0 needed 2 retro rounds, m1 needed 4,
> m2 needed 2, m3 needed 4 — budget at least 2 rounds for m4.
>
> Worktree `/home/luiz/lab-wt/m4-retro-claude` on branch
> `agents/m4/retro`, forked off `agents/m4/driver` at
> `0a0e824` (Phase-3 complete + the carveout-decomposition
> fix). 28 plan-row commits (`8c4a1f1..462f8e7`) land in 1:1
> correspondence with `commits.md` round 3 ratification, plus
> the one Phase-3 follow-up fix commit (`0a0e824`) that
> restored the c27 demo-bar headline test under the real
> sandbox (see §3.9). `scope.md` round 6 converged after 6 pi
> review rounds; `commits.md` round 3 ratified after 3 pi
> review rounds (m3's brackets were 22 / 9; m2's 8 / 4).
>
> Companion: `manual-validation.md` (c28). Owner-facing
> interactive `rfl chat` recording, macOS CI run URL, and
> branch push are deferred to the post-retrospective driver
> sweep (matches the m3 round-1 retrospective shape).
>
> Ratification gates pending at this draft:
> 1. ⏳ Pi adversarial review of this document.
> 2. ⏳ Anticipated drift follow-up commits (§4) — Stream A
>    `Provider` banner, decisions rows for `request_id` /
>    `Publisher::Provider` / carveout-decomposition rule,
>    `overview.md` §4.6 reserved-env table.
> 3. ⏳ macOS CI green after branch push (hard gate, m3
>    precedent — `scope.md` §"Acceptance summary").
> 4. ⏳ `manual-validation.md` records interactive demo-bar
>    recording.

This is the milestone-level review against `scope.md` round 6
and `commits.md` round 3 per `plans/README.md` Phase 3. The
five sections match m1 / m2 / m3's structure.

> **Numbering note.** `commits.md` enumerates plan rows
> c01–c28. The git log on `agents/m4/driver` carries 28
> implementation commits in 1:1 correspondence (no bundling,
> no docs-only insertions between plan rows during Phase 3),
> plus one trailing fix commit (`0a0e824`) that landed
> after c28 to restore the c27 headline under the real
> Landlock sandbox. Phase-2 docs-iteration commits (scope
> rounds 1–6, the five `pi-review-{1..5}.md` files,
> commits rounds 1–3, the three `commits-pi-review-{1..3}.md`
> files — 20 commits total) all land before c01 and are not
> counted in the plan-row total.

---

## 1. Coverage

`scope.md` §I positive + negative matrices plus the
per-§ test enumerations name on the order of 60 unique test
files across `rafaello-core/tests/`,
`rafaello-mockprovider/tests/`, `rafaello-readfile/tests/`,
and `rafaello/tests/`. **All scope-named behaviours
land**; one test (`broker_publish_provider_topic_to_internal_subscriber.rs`)
moved one commit later than its scope-original placement
(c10 → c11) per `commits-pi-review-1.md` B-2, and one
positive (`frontend_publish_user_message_reemitted_as_core_session_user_message.rs`)
moved from c15 to c18 per pi-1 B-4 — both were
ratification-time relocations, not Phase-3 drift.

The `nix develop --impure --command cargo test
--manifest-path rafaello/Cargo.toml --workspace --features
test-fixture` acceptance gate (verbatim from `scope.md`
§"Acceptance summary") aggregates **608 tests passed; 0
failed; 0 ignored** across **382 test binaries** on Linux
inside the devshell (`/tmp/m4-acceptance.log`, captured
2026-05-11; transcript pending archival in
`manual-validation.md` §1 during the post-retrospective sweep).

The on-disk test inventory at retro time:

| Crate | `tests/*.rs` count |
|-------|-------------------:|
| `rafaello-core/tests/` | 338 |
| `rafaello-mockprovider/tests/` | 8 |
| `rafaello-readfile/tests/` | 6 |
| `rafaello-tui/tests/` | 6 |
| `rafaello/tests/` | 24 |

(Counts include carryover from m0–m3; m4-added files are
the new `provider_*`, `agent_loop_*`, `reemit_*`,
`broker_publish_provider_*`, `broker_register_provider_*`,
`broker_publish_core_*_taint_*`, `broker_internal_subscriber_*`,
`broker_acl_auto_publish*`, `frontend_publish_user_message_*`,
`frontend_user_message_missing_request_id_rejected`,
`frontend_register_with_broker`, `cross_*`, the eight
`mockprovider_*` and six `readfile_*` integration tests,
the new `rfl_chat_*` orchestration negatives + the
headline `rfl_chat_demo_bar_read_file`, and the
`tui_sends_test_message_after_ready` env-hook assertion.)

### Scope-vs-landed file-name reconciliation

Two scope-named files moved across the c<NN>/c<NN+1>
boundary at `commits.md` ratification (pi-1 B-2 / B-4); no
mid-Phase-3 renames or splits occurred. Both moves are
recorded in `commits.md`'s round-1→round-2 trajectory and
are not coverage gaps.

| `scope.md` test name | Landed file | Landed commit | Reason |
|----------------------|-------------|---------------|--------|
| `broker_publish_provider_topic_to_internal_subscriber.rs` | same | c11 (`5f60cb9`) | scope drafted it for c10 (`handle_provider_publish`), but the test exercises the c11 `subscribe_internal` channel; pi-1 B-2 moved it to c11 to match the dependency. c10 instead lands a `#[cfg(test)]` accessor on the placeholder `notify_internal_subscribers` drain (later replaced in c11). |
| `frontend_publish_user_message_reemitted_as_core_session_user_message.rs` | same | c18 (`61209e7`) | scope drafted it for c15 (frontend ACL extension), but the re-emit pipeline only exists at c17/c18. pi-1 B-4 moved the re-emit assertion to c18; c15 keeps the grant-only sibling `frontend_publish_user_message_accepted_by_broker.rs`. |

### Positive matrix verification

The §I positive matrix is satisfied. Per-row mapping (only
those introduced by m4 — m3-carryover and m2-carryover
positives are not re-listed):

| `scope.md` positive test | Landed file | Commit |
|--------------------------|-------------|--------|
| `broker_register_provider_happy_path.rs` | same | `f2c07ad` (c09) |
| `broker_publish_provider_topic_to_internal_subscriber.rs` | same | `5f60cb9` (c11) |
| `broker_publish_provider_carries_request_id.rs` | same | `0272188` (c10) |
| `broker_publish_core_with_taint_happy_path.rs` | same | `94f5098` (c12) |
| `broker_publish_core_with_taint_excludes_origin_provider.rs` | same | `94f5098` (c12) |
| `reemit_provider_tool_request_to_core_session_tool_request.rs` | same | `61209e7` (c18) |
| `reemit_plugin_tool_result_to_core_session_tool_result.rs` | same | `61209e7` (c18) |
| `reemit_frontend_user_message_to_core_session_user_message.rs` | same | `61209e7` (c18) |
| `reemit_user_message_synthesises_user_taint.rs` | same | `61209e7` (c18) |
| `reemit_user_message_discards_frontend_supplied_taint.rs` | same | `61209e7` (c18) |
| `frontend_user_message_missing_request_id_rejected.rs` | same | `0272188` (c10) (broker-side) + `67442eb` (c15) (re-confirmed under the F-extended ACL) |
| `agent_loop_dispatches_tool_request_to_target_plugin.rs` | same | `9036e22` (c19) |
| `agent_loop_persists_user_message_entry.rs` | same | `9036e22` (c19) |
| `agent_loop_persists_assistant_message_entry.rs` | same | `9036e22` (c19) |
| `agent_loop_persists_tool_call_entry.rs` | same | `9036e22` (c19) |
| `agent_loop_persists_tool_result_entry.rs` | same | `9036e22` (c19) |
| `provider_plugin_spawns_through_supervisor.rs` | same | `c0cb6e2` (c14) |
| `frontend_register_with_broker.rs` (m3 retro §5.9 closer) | same | `67442eb` (c15) |
| `frontend_publish_user_message_reemitted_as_core_session_user_message.rs` | same | `61209e7` (c18) |
| `broker_provider_event_not_fanned_to_external_subscribers.rs` | same | `0272188` (c10) |
| `broker_internal_subscriber_unregister_on_drop.rs` | same | `5f60cb9` (c11) |
| `broker_internal_subscriber_drops_event_when_full.rs` | same | `5f60cb9` (c11) |
| `broker_internal_subscriber_fires_before_external_fan_out.rs` | same | `5f60cb9` (c11) |
| `broker_acl_auto_publishes_tool_result_topic.rs` | same | `439ddbc` (c06) |
| `broker_acl_auto_publish_absent_for_non_tool_plugin.rs` | same | `439ddbc` (c06) |
| `provider_assistant_message_in_reply_to_missing_rejected.rs` | same | `0272188` (c10) |
| `provider_assistant_message_in_reply_to_stale_id_rejected.rs` | same | `0272188` (c10) |
| `provider_tool_request_in_reply_to_stale_id_rejected.rs` | same | `0272188` (c10) |
| `provider_tool_request_in_reply_to_user_message_id_rejected.rs` | same | `0272188` (c10) |
| `provider_assistant_message_in_reply_to_user_message_id_accepted.rs` | same | `0272188` (c10) |
| `mockprovider_manifest_compiles.rs` | same | `355d76e` (c20) |
| `mockprovider_emits_tool_request_for_read_file_pattern.rs` | same | `5e2c971` (c21) |
| `mockprovider_strips_trailing_punctuation_from_path.rs` | same | `5e2c971` (c21) |
| `mockprovider_records_request_id_to_path_mapping.rs` | same | `5e2c971` (c21) |
| `mockprovider_emits_echo_assistant_message_on_no_match.rs` | same | `5e2c971` (c21) |
| `mockprovider_emits_assistant_message_on_tool_result.rs` | same | `5e2c971` (c21) |
| `mockprovider_handles_multibyte_utf8_path.rs` | same | `5e2c971` (c21) |
| `mockprovider_multi_turn_cites_prior_tool_result_id.rs` | same | `5e2c971` (c21) |
| `readfile_manifest_compiles.rs` | same | `a3dc257` (c22) |
| `readfile_returns_content_for_existing_file.rs` | same | `e3fa97c` (c23) |
| `readfile_errors_for_missing_file.rs` | same | `e3fa97c` (c23) |
| `readfile_errors_for_non_utf8.rs` | same | `e3fa97c` (c23) |
| `readfile_errors_for_outside_project_root.rs` | same | `e3fa97c` (c23) |
| `readfile_lockin_denies_outside_grant.rs` (pi-1 H-3) | same | `e3fa97c` (c23) |
| `rfl_chat_demo_bar_read_file.rs` (headline) | same | `bda2682` (c27) |

### Negative matrix verification

The §I negative matrix is satisfied. The six roadmap-row
negative classes all land:

| `scope.md` negative class | Landed file(s) | Commit |
|---------------------------|---------------|--------|
| `tool_result` missing `in_reply_to` rejected | `broker_plugin_tool_result_missing_in_reply_to_rejected.rs` (m2 carryover, regression-checked) + `reemit_plugin_tool_result_missing_in_reply_to_rejected.rs` | `0272188` (c10) + `61209e7` (c18) |
| Provider tool_request with stale/unknown id fails closed | `broker_provider_tool_request_missing_in_reply_to_rejected.rs`, `broker_provider_tool_request_stale_id_rejected.rs` | `0272188` (c10) |
| Tool plugin called directly (not via core re-emission) doesn't reach dispatch | `cross_plugin_tool_request_blocked_at_broker.rs`, `cross_provider_request_to_tool_only_routes_via_core.rs` | `0272188` (c10) + `9036e22` (c19) |
| Tool requested outside grant denied at lockin | `readfile_lockin_denies_outside_grant.rs` (lockin path) + `readfile_errors_for_outside_project_root.rs` (plugin-level ancestor check) | `e3fa97c` (c23) |
| Bus event missing taint envelope rejected | `broker_publish_core_session_tool_request_missing_taint_rejected.rs`, `broker_publish_core_session_tool_result_missing_taint_rejected.rs` | `94f5098` (c12) |
| Plugin-supplied taint discarded/replaced | `reemit_discards_plugin_supplied_taint_on_core_session_tool_request.rs`, `broker_provider_tool_request_with_supplied_taint_discards.rs` | `61209e7` (c18) + `0272188` (c10) |

Plus the m2-supervisor symmetry tests:

| `scope.md` symmetry test | Landed file | Commit |
|--------------------------|-------------|--------|
| `broker_publish_provider_id_segment_mismatch_rejected.rs` | same | `0272188` (c10) |
| `broker_publish_provider_two_segment_topic_rejected.rs` | same | `0272188` (c10) |
| `broker_publish_provider_unknown_namespace_rejected.rs` | same | `0272188` (c10) |
| `broker_publish_provider_outside_grant_rejected.rs` | same | `0272188` (c10) |
| `broker_register_provider_unknown_canonical_rejected.rs` | same | `f2c07ad` (c09) |
| `broker_register_provider_duplicate_rejected.rs` | same | `f2c07ad` (c09) |

### CLI orchestration negatives

`scope.md` §C14 enumerates `rfl chat` orchestration
negatives. All five land in c24, plus a sixth
(`rfl_chat_tool_spawn_failure_propagates`) added in c25
per pi-1 H-4:

| `scope.md` CLI negative | Landed file | Commit |
|-------------------------|-------------|--------|
| `rfl_chat_missing_lock_errors.rs` | same | `a01f565` (c24) |
| `rfl_chat_invalid_lock_errors.rs` | same | `a01f565` (c24) |
| `rfl_chat_lock_validation_fails.rs` | same | `a01f565` (c24) |
| `rfl_chat_no_active_provider_errors.rs` | same | `a01f565` (c24) |
| `rfl_chat_provider_spawn_failure_propagates.rs` | same | `8dbdfbb` (c25) |
| `rfl_chat_tool_spawn_failure_propagates.rs` (pi-1 H-4) | same | `8dbdfbb` (c25) |

### Tests added beyond the matrix (examples)

The list below is illustrative, not exhaustive. Other
non-matrix tests required by individual `commits.md`
acceptance lines include compile-surface assertions and
unit-level seam tests:

- `reemit_invalid_taint_emits_reemit_rejected_event.rs`
  (c18 — pi-1 H-1 fault-injection seam exercise) and
  `reemit_unknown_tool_emits_tool_dispatch_rejected_event.rs`
  (c18 — `core.lifecycle.tool_dispatch_rejected` lifecycle).
- `reemit_router_subscribes_and_shuts_down.rs` (c17 —
  `ReemitRouter` lifecycle).
- `broker_fan_out_populates_provider_observed_results.rs`
  (c12 — observed-id-set side effect of `publish_core_with_taint`).
- `broker_error_provider_variants_round_trip.rs` (c08).
- `bus_event_serializes_provider_publisher_identity.rs`
  (c07 — wire-schema round-trip under the `request_id` cutover).
- `rfl_chat_eager_spawns_provider_and_tool_then_shuts_down_cleanly.rs`
  (c26 — pi-1 B-6 wire-up smoke test that the c27 headline
  depends on).
- `tui_sends_test_message_after_ready.rs` (c16 — proves the
  env-hook fires at the right point in the TUI lifecycle).
- `env_scrubber_rejects_rfl_provider_id.rs` (c05 — pi-1 M-1
  M1.1 enforcement).
- `supervisor_spawn_reserved_rfl_provider_id_in_set_refused.rs`
  (c05 — supervisor-side mirror of the env-scrubber rule).

All extras are additive; none replace a scope-named
behaviour.

### Coverage verdict

No behavioural coverage loss recorded. The m2 row-39
synthetic stub `supervisor_spawn_provider_lock_refused.rs`
was deleted in c14 (`c0cb6e2`) per the synthetic-stub-
successor rule (`plans/README.md` §"Patterns from prior
milestones"); its successor
`provider_plugin_spawns_through_supervisor.rs` lands in
the same commit. m4 ships no Phase-3 test deletions
without a same-commit successor.

The 608 tests / 0 failed result on Linux satisfies
`scope.md` §"Acceptance summary"'s Linux `cargo test`
bullet; the macOS CI green hard gate remains pending the
post-retrospective branch push (m3 precedent); the cargo
build green and cargo doc warning-free bullets are
captured in §"Acceptance summary" footer below.

---

## 2. Drift against overview / decisions / stream RFCs

The drift items below were **anticipated** by `scope.md`
§"Acceptance summary" — five bullets pre-named the items
the m4 retrospective was expected to address. Following
the m1 / m2 / m3 pattern, this retrospective records the
canonical fix; the actual `decisions.md` rows / overview
patches / Stream A banner land as separate follow-up
commits on this branch before milestone close (per the
authoring conventions, `Updates land as commits on the
milestone branch before merge`).

### 2.1 Stream A security-RFC §10 v1-summary banner (anticipated)

`streams/a-security/rfc-security-model.md` §10 still
describes the round-1 sink-rule wording where overview
§6.2 is now the source of truth. m4 lands a banner-only
patch at the top of the security RFC pointing at overview
§6.2 and `decisions.md` row 9 (the existing row 9 already
describes the user-only-taint sink bypass shape). Body of
the RFC is not retroactively rewritten (m1 / m3 banner
precedent — `plans/README.md` §"Authoring conventions").

### 2.2 `PublisherIdentity::Provider` schema additions to Stream A (anticipated)

Symmetric to m3's banner addition for `Frontend`. Stream
A's wire-schema banner expands from "m3 wire schemas" to
include `PublisherIdentity::Provider { canonical,
provider_id }` once m4 promotes it via the c07 cutover
(`2bbf3e7`). Follow-up commit on this branch updates the
`streams/a-security/rfc-security-model.md` banner.

### 2.3 `decisions.md` row for the `BusEvent.request_id` rollout (anticipated)

c07 (`2bbf3e7`) lands the `BusEvent.request_id:
Option<JsonRpcId>` workspace cutover. Per pi-5 L-1 the
new ratified row records that `request_id` is **mandatory
on the broker** for inbound publishes whose topic suffix
is `.tool_request`, `.tool_result`, `.assistant_message`,
or `.user_message`; the field is `Option` only because m2
non-`request_id`-bearing events (`core.lifecycle.*`,
`core.session.entry.finalized`, etc.) are still served
the same `BusEvent` shape with `None`. The
landed enforcement files
(`broker_plugin_tool_result_missing_request_id_rejected.rs`,
`broker_publish_core_session_user_message_missing_request_id_rejected.rs`,
`broker_provider_tool_request_missing_request_id_rejected.rs`,
`broker_frontend_user_message_missing_request_id_rejected.rs`)
prove the per-handler enforcement. Follow-up commit on
this branch adds the `decisions.md` row.

### 2.4 `decisions.md` row for the `Publisher::Provider` variant landing (anticipated)

c07 (`2bbf3e7`) extends `Publisher` with the `Provider`
arm in the same workspace cutover. Per `scope.md`
§"Acceptance summary" this refines decisions row 42
(which already documented the `Publisher`-shaped
reshape on `PublishOutsideGrant` / `InvalidInReplyTo`
during m3). The new row pins the `Provider` variant
shape (canonical id + provider_id string). Follow-up
commit on this branch adds the row.

### 2.5 `overview.md` §4.6 reserved env-vars table — `RFL_PROVIDER_ID` (anticipated)

c05 (`b1bb66e`) extended `rafaello-core`'s
`RESERVED_ENV_VARS` to reject `RFL_PROVIDER_ID`. Per
pi-1 H-1 the overview §4.6 reserved-env-vars table
should record the addition (and the round-2 drop of
`RFL_PROVIDER_ACTIVE` from the proposed reserved list —
m4 does not need it, the active provider is read from
the lock). Follow-up commit on this branch extends
the §4.6 table.

### 2.6 m1 lock-side `check_lock_publish_topic` unknown-namespace gap (m3 retro §2.7 carryover)

m3 retro §2.7 filed this as "m4 may close it if a failure
surfaced; otherwise it stays as runtime-only enforcement
and is re-filed for m5+." **No user-facing failure
surfaced during m4.** The m4 broker rejects unknown
namespaces at runtime via `handle_*_publish` regardless of
the lock-side check; no agent encountered a confusion
caused by lock-side acceptance. Re-filed for m5+ unchanged.

### 2.7 scope.md §C8 stale `[load] eager = true` schema (m4-introduced drift)

Discovered mid-Phase-3 (c20 agent paused on the manifest
shape). `scope.md` round 6 — and the §C8 manifest-fixture
descriptions throughout — uses the obsolete m1 table
syntax `[load] eager = true`. The live m1 manifest schema
expects the **string shorthand** `load = "eager"`, which
is what the landed fixtures use:

```toml
# rafaello/fixtures/rafaello-mockprovider/rafaello.toml
load = "eager"

# rafaello/fixtures/rafaello-readfile/rafaello.toml
load = "eager"
```

Verified via `grep -A2 load rafaello/fixtures/*/rafaello.toml`
at retro time. The driver pre-corrected the c22 prompt to
the live syntax and c20 / c22 landed the shorthand cleanly.

Per `plans/README.md` §"Authoring conventions" stream RFCs
and `scope.md` are not retroactively rewritten; the drift
is recorded here. **Recommended follow-up:** the
post-retrospective sweep should grep `overview.md` and
the Stream F manifest RFC for the same `[load]`-table
syntax and, if found, either patch `overview.md` (per
authoring conventions: `overview.md` is editable) or add
a `decisions.md` row pinning the live `load = "eager"`
string shorthand as canonical. Stream F drift gets
recorded in m5's retro per the standard rule.

### 2.8 Pi-as-diagnostic-tool extension to environment/sandbox failures

The c27 carveout episode (§3.9 below) was an
environment/sandbox failure the per-commit agent had
already produced a clean test for — the test passed in CI
because the kernel/syd combination there did not enforce
the dir-only access-rights rule, but failed locally
under kernel 6.12 / Landlock ABI 6 / syd 3.49.1. The
per-commit agent finished and committed; the **driver**
discovered the local failure during the orchestrator-side
green-bar pass. Pi-as-diagnostic-tool (m2 retro §4.2)
collapsed the diagnosis loop in <30 minutes when the
agent itself was no longer in scope.

**Recommended `plans/README.md` §"Recurring operational
gotchas" addition:** the existing pi-as-diagnostic-tool
bullet covers an in-flight per-commit agent that thrashes;
m4 demonstrates the same pattern works for
**post-commit** sandbox/environment failures the agent
diagnosed but didn't fix (or didn't surface, if local
infra differs from CI). The pattern is the same: copy
the relevant files into a fresh pi worktree, hand pi
concrete hypotheses, ask for verdict + fix. Carveout fix
landed as `0a0e824`.

### 2.9 Verdict

Two of the eight drift items already landed as Phase-3
code commits (§2.6 — m3 §2.7 carryover, no commit needed
because no failure surfaced; §2.7 — manifest fixtures
landed with the live syntax). Two are recorded-only with
no immediate action needed (§2.6 m5+ refile; §2.8 README
recommendation). **The remaining four (§2.1 / §2.2 /
§2.3 / §2.4 / §2.5) are pending follow-up commits** on
this branch before ratification. The carveout-decomposition
fix (§3.9) is also a follow-up candidate for a
`decisions.md` row pinning the rule that
`carveout::decompose_dir` must classify children by
their resource type (dir vs non-dir) and route non-dirs
into `read_paths`, since the rule is load-bearing for
every future plugin that gets a directory grant.

No additional unprompted drift surfaced during the Phase 3
review beyond the five anticipated items + the two new
ones (§2.7 manifest-syntax, §2.8 pi-as-diagnostic
extension).

---

## 3. What landed (per-phase narrative)

Phase 3 ran over multiple orchestrator restarts (the
driver's worktree disk filled twice mid-milestone — see
§4.1 below). Per-commit walltimes ranged from c13's ~3
min (a single defence-in-depth ACL-construction test) to
c10's ~24 min (`handle_provider_publish` + observed-id
maps + 5 tests in one commit per pi-1 H-3 sizing waiver)
to c18's ~17 min (4 reemit directions + 11 tests + the
shared `reemit_test_kit` common module). Total ~28
plan-row commits + 1 fix commit landed in roughly one
wall-clock day of driver activity.

### 3.1 Phase A — Foundation (c01-c06)

Workspace deps + crate scaffolds + m1 back-reaches.

- **c01** (`8c4a1f1`) — workspace `members` cutover with
  placeholder `Cargo.toml` + `src/lib.rs` for
  `rafaello-mockprovider` and `rafaello-readfile` so the
  workspace resolves cleanly before c03/c04 add the bin
  targets. Per pi-2 L-1 the placeholder approach replaces
  the round-1 "members-list-only" wording.
- **c02** (`a9a2bdb`) — `ulid` dep on `rafaello-tui` for
  the c16 `RFL_TUI_TEST_MESSAGE` env-hook id generation.
- **c03 / c04** (`990c40f`, `786848c`) — full deps + bin
  targets + lib skeletons for the two new plugin crates.
- **c05** (`b1bb66e`) — extends `RESERVED_ENV_VARS` to
  reject `RFL_PROVIDER_ID` (M1.1). Lands the new test
  `env_scrubber_rejects_rfl_provider_id.rs` per pi-1 M-1.
- **c06** (`439ddbc`) — m1 grant compiler back-reach: the
  `plugin.<topic-id>.tool_result` auto-publish for tool
  plugins (M1.3, pi-1 B-6). Two tests:
  `broker_acl_auto_publishes_tool_result_topic.rs` (positive)
  + `broker_acl_auto_publish_absent_for_non_tool_plugin.rs`
  (negative).

### 3.2 Phase B — Broker surface (c07-c13)

The m4 broker extensions in dependency order. Largest and
most coupled phase.

- **c07** (`2bbf3e7`) — **monolithic workspace cutover**
  for `BusEvent.request_id: Option<JsonRpcId>` +
  `Publisher::Provider` + `PublisherIdentity::Provider`.
  Per scope §"Internal split" + m0 c08 precedent, this is
  unsplittable: every `BusEvent` constructor site in m2
  / m3 tests + the m3 TUI test harness must update in
  one commit. Adds `bus_event_serializes_provider_publisher_identity.rs`.
- **c08** (`f36cad5`) — typed `BrokerError` variants for
  `MissingRequestId`, `InvalidTaint`, `StaleRequestId`,
  the `Provider`-publisher arms on existing variants.
  Adds `broker_error_provider_variants_round_trip.rs`.
- **c09** (`f2c07ad`) — `register_provider` + RAII
  `ProviderGuard` (B5). Three positives + two negatives
  (`broker_register_provider_*`).
- **c10** (`0272188`) — `handle_provider_publish` +
  `provider_observed_results` / `provider_observed_user_messages`
  maps + per-handler `request_id` enforcement (B6 + B7b +
  B0 enforcement extension to all three handlers per pi-1
  B-3). Largest single commit by test count (~12 tests).
  Per pi-1 H-3 sizing waiver: handler dispatch consumes
  the maps inline so the natural split point (maps vs
  dispatch) is not viable. Includes the c11-replaced
  `notify_internal_subscribers` placeholder drain with a
  `#[cfg(any(test, feature = "test-fixture"))]` accessor
  so c10's negatives can observe inbound provider events
  without c11's channel API.
- **c11** (`5f60cb9`) — `subscribe_internal` RAII primitive
  + the real `mpsc::Sender<BusEvent>` flow that replaces
  c10's drain (CR1 prerequisite). Three lifecycle tests +
  the moved-from-c10 positive
  `broker_publish_provider_topic_to_internal_subscriber.rs`.
- **c12** (`94f5098`) — `publish_core_with_taint` +
  `origin_provider` exclusion (B8 + B10). Two positives +
  the two missing-taint negatives. Side-effect:
  `broker_fan_out_populates_provider_observed_results.rs`
  proves the user-message-fanout populates the `provider_observed_user_messages`
  set, so c12 deletes c10's seed seam (per `commits.md`
  round-3 B-1).
- **c13** (`bbd840c`) — defence-in-depth provider
  publish-id check at `BrokerAcl::compile` (B11).

### 3.3 Phase C — m2 row-39 removal (c14)

- **c14** (`c0cb6e2`) — removes the m2 supervisor's row-39
  refusal (`SpawnError::InvalidPlan { ProviderNotInM2 }`)
  + wires provider broker registration. Synthetic-stub
  successor: deletes
  `supervisor_spawn_provider_lock_refused.rs`, adds
  `provider_plugin_spawns_through_supervisor.rs` as the
  positive successor in the same commit per the
  `plans/README.md` synthetic-stub rule. The
  `provider_bus_publish` fixture mode is extended per pi-1
  H-2 with the `request_id: Some(JsonRpcId::String(<fresh
  ULID>))` + `in_reply_to: Some(vec![])` shape required
  by the new B0 enforcement.

### 3.4 Phase D — Frontend ACL + TUI test-mode hook (c15-c16)

- **c15** (`67442eb`) — extends m3's `tui` frontend ACL
  with `frontend.tui.user_message` publish authority (F1-
  F4) + the m3 retro §5.9 granularity test
  `frontend_register_with_broker.rs`. The grant-only
  `frontend_publish_user_message_accepted_by_broker.rs`
  lands here (per pi-1 B-4); the re-emit assertion
  `frontend_publish_user_message_reemitted_as_core_session_user_message.rs`
  defers to c18.
- **c16** (`d5bef52`) — `RFL_TUI_TEST_MESSAGE` env-hook in
  `rafaello-tui` with ulid-based `request_id` generation
  (T1). Adds `tui_sends_test_message_after_ready.rs` to
  prove the env-hook fires at the right lifecycle point.

### 3.5 Phase E — Re-emit pipeline + agent loop (c17-c19)

- **c17** (`8d00102`) — `ReemitRouter` scaffold +
  active-provider scoping (CR1, CR6, CR7). Includes the
  pi-1 H-1 `with_test_fault_injector` seam. Two
  lifecycle tests:
  `reemit_router_subscribes_and_shuts_down.rs` +
  `reemit_invalid_taint_emits_reemit_rejected_event.rs`.
- **c18** (`61209e7`) — per-direction re-emit logic
  (CR2-CR5) + the `reemit_test_kit.rs` common module per
  pi-1 M-3. Lands all four re-emit-direction tests + the
  user-taint synthesis pair + the
  `reemit_unknown_tool_emits_tool_dispatch_rejected_event.rs`
  lifecycle test. Eleven tests total — the second-largest
  commit by test count.
- **c19** (`9036e22`) — `AgentLoop` + tool dispatch +
  entry persistence (AL1-AL8 + TD1-TD3). Five
  `agent_loop_persists_*` tests +
  `agent_loop_dispatches_tool_request_to_target_plugin.rs`.
  `AgentLoop::new` carries `caps: Capabilities` per pi-1
  B-5; c26 wires `Capabilities::tui_default()`.

### 3.6 Phase F — Plugin fixtures (c20-c23)

- **c20** (`355d76e`) — `rafaello-mockprovider` manifest
  fixture + `openrpc.json` + executable-shim placeholder
  (per pi-1 B-1: live `manifest::validate_with_package`
  canonicalises the entry path so a non-existent file
  fails). Compile-test `mockprovider_manifest_compiles.rs`.
- **c21** (`5e2c971`) — `rafaello-mockprovider` bin
  implementation + the seven integration tests. Includes
  `mockprovider_multi_turn_cites_prior_tool_result_id.rs`
  per pi-3 H-1 (multi-turn correlation coverage) +
  `mock_provider_handle.rs` common module per pi-1 M-3.
- **c22** (`a3dc257`) — `rafaello-readfile` manifest
  fixture + `openrpc.json` + executable shim +
  `readfile_manifest_compiles.rs`.
- **c23** (`e3fa97c`) — `rafaello-readfile` bin
  implementation + five integration tests. Includes the
  pi-1 H-3 lockin denial test
  `readfile_lockin_denies_outside_grant.rs` exercised via
  the `RFL_READFILE_TEST_BYPASS_GUARD=1` env that skips
  the in-plugin ancestor check, proving the sandbox
  refuses the read independently of the plugin-level
  defence.

### 3.7 Phase G — `rfl chat` orchestration (c24-c26)

- **c24** (`a01f565`) — lock-load + V3 + `compile_plugin`
  per plugin + the five orchestration negatives (C1-C14).
  Migrates m3-era `rfl chat` tests per pi-1 B-7 (every
  test tempdir now writes a minimal stub `rafaello.lock`
  and asserts `NoActiveProvider` exit instead of clean
  exit, since m4 makes a provider load-bearing).
- **c25** (`8dbdfbb`) — supervisor construction + eager
  spawn provider + tool + `ProviderSpawnFailed` /
  `ToolSpawnFailed` negatives (`rfl_chat_provider_spawn_failure_propagates.rs`,
  `rfl_chat_tool_spawn_failure_propagates.rs` — the
  latter per pi-1 H-4).
- **c26** (`b909fce`) — `ReemitRouter` start + `AgentLoop`
  start + TUI spawn + wait loop + shutdown wiring +
  the same-commit smoke test
  `rfl_chat_eager_spawns_provider_and_tool_then_shuts_down_cleanly.rs`
  per pi-1 B-6 (proves the wire-up works without
  depending on the c27 demo bar).

### 3.8 Phase H — Demo bar + manual validation (c27-c28)

- **c27** (`bda2682`) — `rfl_chat_demo_bar_read_file.rs`
  headline test with the literal-pinned README bytes per
  pi-2 H2-4. Asserts SQLite `entries` table contents
  (kind + author per pi-1 B-8), the canonical
  `bus.event` stderr lines, and the assistant message
  text equality `"Here's what's in README.md:\nm4 demo
  readme\n"`.
- **c28** (`462f8e7`) — `manual-validation.md`. macOS CI
  URL + interactive recording deferred to the
  post-retrospective sweep.

### 3.9 The c27 carveout fix episode (post-c28 follow-up — `0a0e824`)

The c27 headline test passed cleanly through the
per-commit agent's iterations and through the agent's
own pre-commit `cargo test`, but **failed locally on the
driver-side green-bar pass** under kernel 6.12 / Landlock
ABI 6 / syd 3.49.1 with:

```
incompatible directory-only access-rights: AccessFs(8) / EOPNOTSUPP
```

Root cause (pi-as-diagnostic-tool, <30 min):
`rafaello-core/src/carveout::decompose_dir` was emitting
**every** child of the project root — including regular
files like `README.md` and `flake.nix` — into `read_dirs`.
The supervisor then called `LandlockPathBeneath::read_dir(p)`
on those file resources, and Landlock ABI ≥ 3 rejects the
combination (dir-only access-rights are not valid on
non-directory file descriptors).

**Pre-existing bug:** `decompose_dir` has been emitting
files-as-dirs since m0; m0/m1/m2/m3 never tripped it
because none of those milestones eagerly spawned a tool
plugin with a project-root grant under the real sandbox.
m4's c25 + c27 are the first end-to-end exercise of
that path.

Fix (`0a0e824`, +22 / -14):
`decompose_dir` now classifies each child by
`Metadata::is_dir()` and routes non-directory children
into `read_paths` instead of `read_dirs`. The existing
`carveout_default_workspace_decomposition.rs` test was
updated to assert the new partition shape.

Post-fix verification: `cargo test -p rafaello --test
rfl_chat_demo_bar_read_file` passes in ~15s locally; the
workspace-wide green-bar (608/0/0, §1) confirms no
regression in any other carveout-using test.

This episode demonstrated pi-as-diagnostic-tool extending
beyond the m2 "agent thrashes for 1h+" framing into
"agent committed clean test, environment exposes a
pre-existing bug" territory. Recorded as §2.8 above with
a recommended `plans/README.md` addition.

### 3.10 No item from scope's lists slipped or was cut

All 28 plan rows in `commits.md` round 3 land 1:1. No
mid-Phase-3 restructure (m2 had a c22→c23 relocation in
`d7c7705`) was needed. The two ratification-time test
relocations (§1 reconciliation table) happened during
pi-1 → pi-2 commits review, not during Phase 3.

### 3.11 No m4a / m4b split was triggered

m4's surface (broker provider class + `request_id`
envelope cutover + provider registration + internal-
subscriber primitive + agent loop + re-emit pipeline +
two new plugin crates + `rfl chat` orchestration)
threaded through 28 commits without forcing a milestone
split. The `commits.md` §"m4a / m4b checkpoint"
re-evaluations after c13 and c19 both came up clean.

### 3.12 No extra-scope features added during implementation

Per-commit agents stayed within the row-prescribed scope.
No new public APIs, error variants, or test files beyond
the row enumerations were added by an agent without
explicit scope authorization. The c10 sizing waiver was
ratified in `commits.md` round 1 (pi-1 H-3); it was not
an in-flight Phase-3 expansion.

---

## 4. Process notes for the next milestone driver

### 4.1 Per-commit `target/` dirs blew the disk twice (BLOCKING for next milestone)

Per-commit worktrees under `/home/luiz/lab-wt/m4-cNN/`
each grew their own `rafaello/target/` to **9-23 GB**
(c01..c10 alone aggregated to **~110 GB combined**). Disk
filled twice during Phase 3, both times killing the
orchestrator mid-run and forcing a manual cleanup +
restart. Each restart cost ~5-10 minutes of recovery
context (stash state, identify which commit was in
flight, resume tmux).

**Mitigation adopted mid-milestone:** the driver started
running `rm -rf <wt>/rafaello/target` immediately after
each ff-merge (before removing the worktree itself). With
this mitigation the disk stayed under control through the
remaining ~18 commits.

**Recommended `plans/README.md` §"Recurring operational
gotchas" addition:** explicit step in the per-commit
cleanup contract — "after `git merge --ff-only
agents/m<N>/c<NN>` succeeds, run `rm -rf
<wt>/rafaello/target` before `git worktree remove`."
Without this, any milestone with ≥10 commits on a
machine with a typical-size lab partition will hit
disk-full inside Phase 3.

### 4.2 Pi-as-diagnostic-tool now applies to env/sandbox failures (post-commit)

See §2.8 + §3.9. The existing `plans/README.md` bullet
covers an in-flight per-commit agent that thrashes; m4
extends the pattern to **post-commit, driver-discovered**
sandbox/environment failures the per-commit agent could
not reproduce (because the agent's environment differs
from the orchestrator's, or because the failure depends
on local kernel/syd state that the agent cannot
introspect). Recommended `plans/README.md` extension:
add one sentence to the existing pi-as-diagnostic-tool
bullet — "also applies to environment/sandbox failures
the per-commit agent finished and committed against, but
that surface only on the orchestrator side."

### 4.3 Six pi rounds on scope, three on commits is m4's bracket

m4 took **6 scope review rounds** (m0: 3, m1: 4, m2: 8,
m3: 22) and **3 commits review rounds** (m0: 3, m1: 3,
m2: 4, m3: 9). This is the **smallest bracket since m1**
despite m4 introducing a new broker publisher class +
two new plugin crates + an agent loop. Three factors:
(1) m3's six-pi-round scope had already shaped the
session/replay semantics m4 inherits, (2) the
`request_id`-cutover risk (`scope.md` §Risks 1) was
visible from round 1 and structured around the m0 c08
single-commit pattern from the start, (3) the
synthetic-stub-successor rule was already canonical from
m2 so c14's row-39 removal landed without new policy
debate. **For sizing future milestones:** the m4
bracket is what to expect when prior milestones have
already established the design vocabulary; m3-style
bracket (22+ scope rounds) signals that the milestone is
introducing genuinely new architectural concepts (m3's
TUI + sessions + rendering were all new). m5 introduces
sinks + confirmation + user_grants — likely back to a
larger bracket.

### 4.4 Per-commit-prompt inlining held (m2 / m3 carryover)

The pattern established in m2 — paste the full
`commits.md` row verbatim into the per-commit prompt,
plus the relevant `scope.md` excerpts inlined — held
across all 28 m4 commits. No agent re-read `commits.md`.
No mid-milestone bundling drift like m1's c31+c32 episode.

### 4.5 `Cargo.lock` stash-before-ff-merge held (m2 / m3 carryover)

The m2 retro §4.5 mitigation — `git stash push
rafaello/Cargo.lock` before each ff-merge, then `git
stash drop` — held across all 28 m4 ff-merges. No
ff-merge silently aborted by a dirty Cargo.lock.

### 4.6 Per-commit cleanup contract held (m2 / m3 carryover, with the §4.1 addition)

Worktree creation in `/home/luiz/lab-wt/m4-cNN/`, prek
symlink, `direnv allow .`, tmux claude session spawn,
`tmux-wait` on `"esc to interrupt"`, ff-merge with
Cargo.lock stash, **`rm -rf <wt>/rafaello/target` (§4.1
new step)**, worktree removal — pattern held without
exception across all 28 commits once the §4.1 step was
added.

### 4.7 c10 H-3 sizing waiver held under Phase 3 stress

`commits.md` round 1 pi-1 H-3 ratified a sizing waiver
for c10 (~300 LoC handler + ~150 LoC tests + maps/
bookkeeping; splitting maps from dispatch not viable).
The c10 commit landed at the predicted size; no agent-
side pressure to split further. The waiver shape
(commit-body LoC declaration + pi-ratified rationale)
worked. m5+ should reuse this shape when a single
acceptance row's natural granularity exceeds the m0 §4.1
single-cutover budget.

### 4.8 Phase 3 walltime profile

Per-commit agent walltimes ranged from ~3 min (c13 — a
single defence-in-depth ACL-construction test) to ~24
min (c10 — provider intake + observed-id maps + 5
tests) to ~17 min (c18 — 4 reemit directions + 11 tests
+ a common module). Median ~10-12 min. The two disk-full
restarts added ~15-20 min recovery overhead each. Total
Phase-3 wall-clock was roughly one driver day.

---

## 5. Coverage gaps + owner-accepted carveouts

### 5.1 Stale-correlation enforcement on `plugin.<id>.tool_result.in_reply_to` (scope §"Out of scope" carryover)

`scope.md` §"Out of scope" pins this as a deliberate
gap: the m4 broker has no per-plugin outstanding-tool_request
map, so a tool plugin can publish a `tool_result` citing
an `in_reply_to` the broker has never routed and m4
accepts it. The downstream effect surfaces as the
provider failing to find the matching in-flight request
on its side. m5 adds the agent-loop outstanding map
when it lands the sink-confirmation gate (which needs
the same data structure). Owner-accepted at scope
ratification (round 6).

### 5.2 macOS CI green is a hard ratification gate (scope §"Acceptance summary" carryover)

m3 made macOS CI a hard ratification gate; m4 inherits
the gate. **Status: pending the post-retrospective push
to GitHub.** The driver captures the macOS run URL in
`manual-validation.md` once the branch pushes and the
workflow completes. m4 introduces no new platform-
specific syscalls (the agent loop uses tokio + existing
fittings transport; the new plugin crates have no FS-
syscall paths beyond standard Rust I/O); default
expectation is macOS CI green from day one.

### 5.3 Interactive `rfl chat` recording (scope §"Acceptance summary" carryover)

`scope.md` §"Acceptance summary" requires
`manual-validation.md` to record an interactive
`rfl chat` run against the fixture lock that
demonstrates the demo bar (user types "what's in
README.md", sees the file's contents rendered as an
assistant message). c28 (`462f8e7`) lands the
`manual-validation.md` skeleton; the actual recording
is deferred to the post-retrospective driver sweep.
Owner may accept the c27 headline test's mechanical
green as substitute coverage (m3 §5 precedent), but
the default expectation is a recorded run.

### 5.4 No flakes observed in the recorded run

The 608-test run captured 2026-05-11 inside the devshell
on Linux returned 0 failures across 382 test binaries.
No retries needed. The fixture-self-timeout patch from
m3 §2.9 (`RFL_FIXTURE_MAX_LIFETIME`) extended into
`rfl-mockprovider` and `rfl-readfile` per `scope.md`
§Risks 7; no orphan-fixture-process tax was billed
during the m4 walk.

### 5.5 No clippy warnings suppressed by agents

All per-commit walks closed clippy clean; no
`#[allow(...)]` was introduced by an agent during
Phase 3. The pre-commit clippy hook (`cargo clippy --
-D warnings`) gated every commit.

### 5.6 No `cargo doc` warnings (regression check)

`cargo doc --workspace --no-deps` returns clean (the m2
fix from m2 retro §5.2 + m3's confirmation continue to
hold under m4's surface expansion).
`/tmp/m4-doc.log` captured 2026-05-11.

### 5.7 m1 `check_lock_publish_topic` unknown-namespace gap re-filed (m3 retro §2.7 closure)

See §2.6. No user-facing failure surfaced during m4;
re-filed for m5+. Owner-accepted gap.

### 5.8 Lazy-load not exercised (scope §"Out of scope" carryover)

m4 eager-spawns every installed plugin via §C7/C8; both
fixture manifests carry `load = "eager"`. The
`load.triggers.kind = "tool"` path is m1-validated but
not exercised by any m4 test. `rfl plugin start
--skip-eager` is m5+ territory. Owner-accepted at scope
ratification.

### 5.9 `FrontendSupervisor` lock-correspondence claim (m2 retro §2.6 / m3 retro §2.8 carryover)

Same v2 nice-to-have. No m4 work; deferred to v2.

---

## Follow-up commits on this branch

Per the m1 / m2 / m3 precedent, the drift items in §2
land as **separate commits on this branch before
milestone close**. The list below pre-names them; items
1–5 are docs-only/banner-only patches addressing §2
drift; item 6 is a recommended `plans/README.md`
extension; item 7 is a candidate `decisions.md` row for
the carveout-decomposition rule (the code already landed
at `0a0e824`).

1. `docs(rafaello-streams-a): v1-status banner — point at
   overview §6.2 + decisions row 9` — addresses §2.1.
2. `docs(rafaello-streams-a): banner expand — include
   PublisherIdentity::Provider { canonical, provider_id }`
   — addresses §2.2.
3. `docs(rafaello-decisions): request_id mandatory on
   tool_request / tool_result / assistant_message /
   user_message broker handlers` — addresses §2.3.
4. `docs(rafaello-decisions): Publisher::Provider variant
   — refines row 42` — addresses §2.4.
5. `docs(rafaello-overview): §4.6 — RFL_PROVIDER_ID
   addition to reserved env-vars table; drop
   RFL_PROVIDER_ACTIVE from proposed list` — addresses
   §2.5.
6. `docs(rafaello-plans-README): per-commit target/ dir
   cleanup + pi-as-diagnostic for env/sandbox failures`
   — addresses §4.1 + §2.8.
7. (optional) `docs(rafaello-decisions): carveout
   decomposition routes non-directory children into
   read_paths, not read_dirs` — pins the rule from the
   `0a0e824` fix so future plugins with directory grants
   inherit the invariant.

Items §2.6 (m1 lock-side gap, no failure surfaced) and
§2.7 (manifest-syntax drift, fixtures landed correctly)
are recorded-only — no follow-up commit needed in m4.

§2.7's `recommended-follow-up` to grep `overview.md` /
Stream F for the same `[load]`-table syntax is a
pre-merge action item for the driver, not a separate
commit.

---

## Acceptance summary check

The table maps each `scope.md` §"Acceptance summary"
bullet to its current status. The exact cargo commands
quoted match scope verbatim — note `--manifest-path
rafaello/Cargo.toml --workspace --features test-fixture`
(no `rafaello-core/` prefix; `test-fixture` is the
workspace-feature alias defined on `rafaello-core` and
exposed through the workspace member resolution).
`--workspace --bins --features rafaello-core/test-fixture`
is the build gate's exact form (the explicit
`rafaello-core/` prefix is required there because
`--bins` does not auto-enable the feature).

| `scope.md` §"Acceptance summary" bullet | Status |
|-----------------------------------------|--------|
| Every named test in §"Positive" / §"Negative" matrices implemented and passing | ✅ §1 (with 2 ratification-time relocations recorded) |
| `cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture` green on Linux inside devshell | ✅ **608 passed / 0 failed / 0 ignored** across 382 test binaries, captured 2026-05-11 (`/tmp/m4-acceptance.log`) |
| Demo-bar headline `rfl_chat_demo_bar_read_file.rs` green | ✅ `test rfl_chat_demo_bar_read_file ... ok` (`/tmp/m4-acceptance.log`); restored under the real Landlock sandbox by `0a0e824` (carveout decomposition fix) |
| **macOS CI green** (hard gate) | ⏳ pending post-retrospective branch push |
| `cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture` green | ✅ captured 2026-05-11 (`/tmp/m4-build.log` — `Finished dev profile`) |
| `cargo doc --manifest-path rafaello/Cargo.toml --workspace --no-deps` warning-free | ✅ captured 2026-05-11 (`/tmp/m4-doc.log` — `Finished dev profile`, no warnings) |
| `manual-validation.md` records interactive `rfl chat` demo + macOS CI URL | ⏳ skeleton landed at c28 (`462f8e7`); recording + CI URL pending the post-retrospective driver sweep |
| Stream A §10 v1-summary banner patch | ⏳ §2.1 — follow-up commit |
| `PublisherIdentity::Provider` Stream A schema additions | ⏳ §2.2 — follow-up commit |
| `decisions.md` row for `BusEvent.request_id` rollout | ⏳ §2.3 — follow-up commit |
| `decisions.md` row for `Publisher::Provider` variant | ⏳ §2.4 — follow-up commit |
| m1 `check_lock_publish_topic` unknown-namespace gap | §2.6 — re-filed for m5+, no commit needed |
| Provider-side env-var documentation in `overview.md` §4.6 | ⏳ §2.5 — follow-up commit |
| `retrospective.md` written with anticipated drift addressed | ✅ this document (round 1, awaiting pi adversarial review) |

---

**m4 round-1 retrospective complete 2026-05-11.** Pi
adversarial review next per `plans/README.md` §"Patterns
from prior milestones". Six follow-up commits pre-named in
the list above land on this branch after the pi review
converges; the macOS CI run + interactive demo recording
land in `manual-validation.md` once the branch pushes.
m5 inherits: the new `Provider` publisher class + envelope
machinery, the `subscribe_internal` primitive and
`ReemitRouter`, the `AgentLoop` + tool-dispatch wiring,
the `frontend.tui.user_message` ACL grant, the
`RFL_TUI_TEST_MESSAGE` env-hook, the two new plugin
fixtures (`rafaello-mockprovider` + `rafaello-readfile`),
the `rfl chat` four-level orchestration tree, and the
post-fix carveout decomposition that classifies children
by resource type.
