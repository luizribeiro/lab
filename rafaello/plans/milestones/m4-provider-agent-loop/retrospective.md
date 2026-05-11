# m4 — provider fixture + secure agent loop + one read-only tool — retrospective

> **Status: ratified and closed 2026-05-11 by the owner.**
> Round 4 (with pi-review-4 polish nits folded as
> `190f5d7`) was pi-ratified at zero blockers in
> `retrospective-pi-review-4.md` (`8a8bd6d`); §5
> interactive recording owner-accepted in lieu of a
> screen capture (mechanical coverage via the c27
> headline test `rfl_chat_demo_bar_read_file`; recording
> deferred to m6 where `rfl init` lands). The 71-commit
> m4 cycle (28 plan-row commits + carveout fix +
> retrospective rounds + drift commits + polish folds)
> rebased and ff-merged onto `rafaello-v0.1` for
> linear history per `decisions.md` row 33. **macOS CI
> green captured 2026-05-11 as run `25655924846`
> against the RATIFIED commit `0cc1405`** (both
> Linux + macOS jobs of `rafaello.yml` passed). Round-3 pi review (3b + 2n) was closed by
> the round-4 patch summary immediately below; round 2
> (2b + 2n) was closed by the round-3 patch summary;
> round 1 (5b + 2n) was closed by the round-2 patch
> summary. m0 needed 2 retro rounds, m1 needed 4, m2
> needed 2, m3 needed 4, m4 needed 4.
>
> Worktree `/home/luiz/lab-wt/m4-retro-r4` on branch
> `agents/m4/retro-r4`, forked off `agents/m4/driver` at
> `442731b` (the round-3 retrospective commit, after
> pi-review-3 landed on the driver branch). 28
> plan-row commits (`8c4a1f1..462f8e7`) land in 1:1
> correspondence with `commits.md` round 3 ratification,
> plus the Phase-3 follow-up fix commit (`0a0e824`) that
> restored the c27 demo-bar headline test under the real
> sandbox (see §3.9), plus the round-2 follow-up commits
> (drift, bus-rs rustdoc fix, manual-validation refresh)
> and the round-3 follow-up commits (Stream A banner +
> decisions row 44 topic_id fixes, manual-validation §5
> wording alignment) listed below.
>
> `scope.md` round 6 converged after 6 pi review rounds;
> `commits.md` round 3 ratified after 3 pi review rounds
> (m3's brackets were 22 / 9; m2's 8 / 4).
>
> Companion: `manual-validation.md` — refreshed in
> round 2 to inline the post-`0a0e824` Linux
> test/build/doc transcripts and drop the stale
> "Known local environment issue" section (commit
> `844c17d`); round 3 also softens §5 owner-acceptance
> wording to match this retrospective's §5.3 (commit
> `adf763f`, pi-r2 N1). Owner-facing interactive `rfl
> chat` recording and macOS CI run URL remain deferred
> to the post-retrospective driver sweep (matches the
> m3 round-1 retrospective shape).
>
> **Round-4 patch summary (pi-review-3 closure):**
> - B1 (decisions row 44 rationale inverted `provider_id`
>   vs `session.provider_active`) → row 44 rationale
>   corrected to match the live code: the active provider
>   plan is selected by **canonical id**
>   (`lock.session.provider_active: Option<String>` parsed
>   via `CanonicalId::parse` and looked up in
>   `compiled_plugins`); `provider_id` is the separate
>   public-namespace segment populated from
>   `entry.bindings.provider_id` into `PluginAcl.provider_id`
>   by `broker_acl::compile` (commit `63abf51`).
> - B2 (Stream A §5 banner kept stale provider API claims)
>   → banner corrected to the live signature
>   `Broker::register_provider(canonical: CanonicalId, peer:
>   PeerHandle) -> Result<RegisteredProvider, BrokerError>`,
>   the `RegisteredProvider` RAII type name, the
>   `bindings.provider: bool` + `bindings.provider_id:
>   Option<String>` lock shape (no `class = "provider"`
>   discriminator), and the c13 mismatch check located in
>   `Broker::new` ACL revalidation as
>   `BrokerError::InvalidTopic` (exercised by
>   `broker_construct_with_provider_publish_id_mismatch_rejected.rs`),
>   not at `BrokerAcl::compile` (commit `41768a5`).
> - B3 (§5.5 still incomplete + `m4_lock_fixture.rs`
>   misdescribed) → §5.5 below adds the c14 production
>   `#[allow(dead_code)]` on `supervisor.rs:176`
>   `SpawnRegistration` (held only for RAII drop; same
>   class as the c09 `bus.rs:101` `ProviderConn` allow),
>   bumps the production-site count to **4**, and
>   corrects the `m4_lock_fixture.rs` entry from
>   "module-level `#![allow(dead_code)]`" to the actual
>   "two item-level function `#[allow(dead_code)]`"
>   shape (one on `write_stub_lock` from c24, one on
>   `write_empty_lock` from c25).
> - N1 (round labels stale) → status frontmatter bumped
>   to round 4; `manual-validation.md` round-label
>   refresh follows in a separate commit.
> - N2 (retrospective uses stale `ProviderGuard` name) →
>   §3.2 c09 narrative + §5.5 `bus.rs:101` rationale now
>   say `RegisteredProvider` (the live public type).
>
> **Round-3 patch summary (pi-review-2 closure):**
> - B1 (`PublisherIdentity::Provider` docs omit live
>   `topic_id` field) → decisions row 44 patched to
>   `{ canonical, provider_id, topic_id }` with softer
>   rationale (commit `fdac914`); Stream A §5 banner
>   extended to record the three-field shape and cite
>   the c07 schema test (commit `2e87e44`); §2.2 below
>   updated inline.
> - B2 (§5.5 under-reports m4 `#[allow]` suppressions) →
>   §5.5 expanded below to a complete production + test
>   inventory (3 production sites + 1 test-helper site +
>   the standard `tests/common/` `#![allow(dead_code)]`
>   pattern), with per-site rationale and the c18 / c19
>   `clippy::result_large_err` choice declared as a
>   deferred follow-up rather than a fix in this round.
> - N1 (`manual-validation.md` §5 overstates owner
>   acceptance) → §5 reworded to "owner *may* accept"
>   matching retro §5.3 (commit `adf763f`).
> - N2 (decisions row 44 rationale shaky on "same
>   canonical installed twice") → softened in commit
>   `fdac914` to the concrete m4 need (provider topics
>   live in the public provider-id namespace; canonical
>   ids identify installed packages).
>
> **Round-2 patch summary (pi-review-1 closure):**
> - B1 (cargo doc warning) → bus.rs intra-doc-link fix
>   (`639d7f0`); doc gate re-run warning-free
>   (`/tmp/m4-doc.log`).
> - B2 (stale `manual-validation.md`) → refreshed with
>   round-2 transcripts and demo-bar status (`844c17d`).
> - B3 (false "no `#[allow]`" claim) → §5.5 reworded to
>   acknowledge the c25 `clippy::too_many_arguments`
>   test-helper suppression.
> - B4 (drift ratification over-marked) → drift follow-up
>   commits landed: `c222087`, `9bd24e3`, `d51caba`,
>   `3a3a917`, `63f6997`, `152813a`; acceptance row now
>   ✅ with hashes; §2 verdict arithmetic corrected to
>   "remaining five".
> - B5 (§2.7 "recorded-only" inconsistency) → §2.7 now
>   cites `decisions.md` row 45 (`3a3a917`) as the
>   authoritative on-disk record, and `overview.md` §5.3
>   was patched to the string shorthand (`152813a`).
> - N1 ("382 test binaries") → reworded to "382 top-level
>   `tests/*.rs` files (Cargo runs ~398 test targets
>   including unit + doc tests)".
> - N2 (c27 "passed in CI") → reworded to "passed in the
>   local Phase-3 agent run" — CI has not been touched
>   yet; macOS / Linux CI captures are post-merge driver
>   tasks.
>
> Ratification gates pending at this revision:
> 1. ⏳ Pi adversarial review of this round-4 document.
> 2. ⏳ macOS CI green after branch push (hard gate, m3
>    precedent — `scope.md` §"Acceptance summary").
> 3. ⏳ `manual-validation.md` records interactive demo-bar
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
failed; 0 ignored** on Linux inside the devshell across
**382 top-level `tests/*.rs` files** (the Cargo run
exercises ~398 test targets including unit + doc tests
per the per-target `Running ...` lines in the log)
(`/tmp/m4-acceptance.log`, captured 2026-05-11 and
re-run 2026-05-11 on the round-2 branch; transcript
inlined in `manual-validation.md` §1).

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
provider_id, topic_id }` once m4 promotes it via the c07
cutover (`2bbf3e7`). The third `topic_id` segment is
parallel to the m2 `Plugin {canonical, topic_id}` identity
shape and is asserted by the c07 schema test
`rafaello-core/tests/bus_event_serializes_provider_publisher_identity.rs`.
Follow-up commits on this branch update the
`streams/a-security/rfc-security-model.md` §5 banner
(round-2 `9bd24e3` introduced the entry as `{canonical,
provider_id}`; round-3 `2e87e44` corrected it to the live
three-field shape) and `decisions.md` row 44 (round-2
`d51caba` ratified row 44; round-3 `fdac914` corrected
`PublisherIdentity::Provider` to include `topic_id` and
softened the rationale per pi-r2 N2).

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
and `scope.md` are not retroactively rewritten; the
authoritative on-disk record is `decisions.md` row 45
(landed on this branch as commit `3a3a917`), and
`overview.md` §5.3's `[load]` table-form entry was
patched to the live string shorthand in commit `152813a`.
Stream F's RFC retains the obsolete examples and m5's
retro records the residual drift per the standard rule.

### 2.8 Pi-as-diagnostic-tool extension to environment/sandbox failures

The c27 carveout episode (§3.9 below) was an
environment/sandbox failure the per-commit agent had
already produced a clean test for — the test passed in
the local Phase-3 agent run (whose ephemeral sandbox
state did not exercise the dir-only access-rights rule
the orchestrator-side green-bar pass tripped on under
kernel 6.12 / Landlock ABI 6 / syd 3.49.1). CI has not
been touched yet — capture is a post-merge driver
task. The
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

Of the eight drift items above, §2.6 (m3 §2.7 carryover)
needed no commit because no failure surfaced, and §2.8
(pi-as-diagnostic-tool extension) is a recorded-only
README recommendation. **The remaining five (§2.1 / §2.2
/ §2.3 / §2.4 / §2.5) landed on this branch as round-2
follow-up commits** (`c222087`, `9bd24e3`, `d51caba`,
`3a3a917`, `63f6997`); §2.7 (manifest-syntax drift) also
landed as `3a3a917` (decisions.md row 45) + `152813a`
(overview §5.3 patch). The carveout-decomposition fix
(§3.9) remains a candidate for a `decisions.md` row
pinning the rule that `carveout::decompose_dir` must
classify children by their resource type (dir vs
non-dir) and route non-dirs into `read_paths`, since the
rule is load-bearing for every future plugin that gets a
directory grant — deferred to a separate post-merge
sweep so this round-2 patch stays scoped to the
pi-review-1 closure.

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
  `RegisteredProvider` (B5). Three positives + two negatives
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
  publish-id check in `Broker::new` ACL revalidation
  (`BrokerError::InvalidTopic`), exercised by
  `broker_construct_with_provider_publish_id_mismatch_rejected.rs`
  (B11).

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
on Linux returned 0 failures across 382 top-level
`tests/*.rs` files (Cargo runs ~398 test targets
including unit + doc tests).
No retries needed. The fixture-self-timeout patch from
m3 §2.9 (`RFL_FIXTURE_MAX_LIFETIME`) extended into
`rfl-mockprovider` and `rfl-readfile` per `scope.md`
§Risks 7; no orphan-fixture-process tax was billed
during the m4 walk.

### 5.5 `#[allow(...)]` suppressions introduced in m4

All per-commit walks closed clippy clean under the
pre-commit clippy hook (`cargo clippy -- -D warnings`).
Round-2 §5.5 mistakenly claimed only one m4
suppression existed and that none were in production
crates (pi-review-2 B2 caught the omission); round-3
§5.5 expanded the list but still omitted the c14
`supervisor.rs:176` production allow and misdescribed
the `m4_lock_fixture.rs` test sites (pi-review-3 B3).
The complete m4-introduced inventory below was
re-audited by walking every `allow(` match in
`crates/rafaello-{core,mockprovider,readfile}` and
`crates/rafaello/tests/common/` with `git blame` —
`git blame` verifies each site lands in an m4 plan-row
commit (m0–m3 carryover sites are not re-listed).

**Production code (4 sites).**

- `rafaello-core/src/reemit/mod.rs:1` —
  `#![allow(clippy::result_large_err)]`. Introduced by
  c18 (`61209e7`). Rationale: the re-emit pipeline's
  `ReemitError` aggregates several variant payloads
  (taint envelopes, publisher identities,
  `BrokerError` arms) that callers match exhaustively,
  so boxing the error would force every call site into
  a `Box<ReemitError>` indirection for no behavioural
  win. This suppression is **deferred to a future
  cleanup pass** rather than addressed in m4: the
  honest choice is to keep `Result<_, ReemitError>` by
  value while m5 explores whether the broker-side
  error hierarchy wants a workspace-wide boxing
  convention. Filed as a §4 follow-up for whoever
  picks up the broker error-shape sweep.
- `rafaello-core/src/agent/mod.rs:1` —
  `#![allow(clippy::result_large_err)]`. Introduced by
  c19 (`9036e22`). Same rationale as the reemit
  module: `AgentLoopError` aggregates dispatch /
  persistence / lifecycle variants that callers match
  on; boxing buys nothing the current shape needs. Same
  deferred-cleanup disposition as the reemit module —
  the two should land together when the workspace
  decides on a boxing convention.
- `rafaello-core/src/bus.rs:101` — `#[allow(dead_code)]`
  on `ProviderConn { peer: PeerHandle }`. Introduced
  by c09 (`f2c07ad`). Rationale: `ProviderConn` is
  registered into `BrokerState::providers` by
  `register_provider` and held alive for as long as
  the `RegisteredProvider` exists; the `peer` field is
  stored so the broker owns the handle (and the I/O
  task it transitively holds) for the registration
  window, but no m4 code path reads it back out of
  the struct yet. m5's confirmation gate is expected
  to read it; until then the field is intentionally
  inert. Field-local `#[allow(dead_code)]` is the
  smallest scope that suppresses the warning without
  hiding future drift.
- `rafaello-core/src/supervisor.rs:176` —
  `#[allow(dead_code)]` on `SpawnRegistration`
  (`enum { Plugin(RegisteredPlugin),
  Provider(RegisteredProvider) }`). Introduced by c14
  (`c0cb6e2`) when the supervisor started discriminating
  plugin vs. provider broker registration so its RAII
  drop releases the right registry slot. Inline source
  rationale `Held only for RAII drop; never read.`
  matches the same class as the c09 `bus.rs:101`
  `ProviderConn` allow: the enum value is constructed
  and stored on the spawned-record solely so the
  `Drop` impl on the wrapped guard fires when the
  record is dropped; no code path reads the variant
  back out. m5's confirmation gate is the natural reader
  (mirrors the `ProviderConn::peer` follow-up); until
  then the suppression is intentional. **Round-3 §5.5
  omitted this site** — pi-review-3 B3 caught the gap.

**Test code (2 + N sites).**

- `rafaello/crates/rafaello/tests/common/m4_install.rs:93`
  — `#[allow(clippy::too_many_arguments)]` on the
  `m4_install` helper constructor. Introduced by c25
  (`8dbdfbb`). Rationale: threads ~9 install-shape
  parameters through one function so the orchestration
  negatives (C14, c25's spawn-failure tests) share one
  fixture builder; collapsing into a builder struct
  was rejected at c25 review as out of scope for a
  test-only helper. Local to the test common module.
- `rafaello/crates/rafaello/tests/common/m4_lock_fixture.rs`
  — **two item-level** `#[allow(dead_code)]` function
  suppressions (one on `write_stub_lock`, c24
  `a01f565`; one on `write_empty_lock`, c25 `8dbdfbb`)
  rather than the module-level `#![allow(dead_code)]`
  the round-3 entry described. The two helpers are
  used by different subsets of the orchestration
  negatives, so each carries a function-local allow
  to keep clippy quiet for the binaries that don't
  call it. Round-3 §5.5 misdescribed the site shape
  (pi-review-3 B3); function-local matches the live
  file.
- Test common-module module-level `#![allow(dead_code)]`
  on the following `tests/common/` files —
  `provider_test_kit.rs` (c10), `reemit_test_kit.rs`
  (c18), `agent_test_kit.rs` (c19),
  `mock_provider_handle.rs` (c21),
  `read_file_tool_handle.rs` (c23), and
  `m4_install.rs` (c25). Rationale: files under
  `tests/common/` are compiled as part of **every**
  integration-test binary in the crate, but each
  binary uses only a subset of the helpers it exports.
  Without the module-level allow, every test target
  that does not exercise a particular helper would fail
  the `-D warnings` clippy gate. This is the standard
  Rust `tests/common/` pattern; round-2 §5.5 omitted it
  as "not interesting", which is a defensible call but
  not what pi-review-2 wanted recorded, so the full
  list lives here for future-milestone reference.

**Follow-up (deferred).** The two production
`result_large_err` allows in `reemit/mod.rs` and
`agent/mod.rs` are filed against a future broker
error-shape sweep; m5 is the natural owner because its
sink-confirmation gate touches both modules' error
types. Until then, suppression is the honest record.

### 5.6 No `cargo doc` warnings (regression check)

`cargo doc --workspace --no-deps` returns warning-free
on the round-2 branch after the bus.rs intra-doc-link
fix (`639d7f0` — replaced a public-doc link to the
private `notify_internal_subscribers` method with prose).
`/tmp/m4-doc.log` re-captured 2026-05-11 contains zero
`warning:` lines. The round-1 draft mistakenly claimed
the gate was clean before the fix; pi-review-1 B1
caught it.

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
milestone close**. Round-2 status (all hashes on
`agents/m4/retro-r2`):

1. ✅ `c222087` — `docs(rafaello-streams-a): §10 banner
   — m4 BrokerAcl provider extensions + auto-publish
   grant for tool plugins` — addresses §2.1.
2. ✅ `9bd24e3` — `docs(rafaello-streams-a): §10 banner
   — PublisherIdentity::Provider + register_provider
   RAII surface` — addresses §2.2.
3. ✅ `d51caba` — `docs(rafaello-decisions): rows 43-44
   — BusEvent.request_id + Publisher::Provider +
   PublisherIdentity::Provider land in m4` — addresses
   §2.3 + §2.4.
4. ✅ `3a3a917` — `docs(rafaello-decisions): row 45 —
   load = "eager" is the live m1 manifest syntax (table
   form reserved for event-triggered lazy)` —
   addresses §2.7.
5. ✅ `63f6997` — `docs(rafaello-overview): §4.6
   reserved-env table — add RFL_PROVIDER_ID` —
   addresses §2.5.
6. ✅ `152813a` — `docs(rafaello-overview): manifest
   field — load string shorthand (live m1 schema)` —
   addresses §2.7's overview reference.

Round-2 also landed three companion commits outside §2:
`639d7f0` (bus.rs rustdoc intra-doc-link fix closing
pi-review-1 B1), and `844c17d`
(`manual-validation.md` refresh closing pi-review-1 B2),
plus `<this-commit>` (the round-2 retro patch itself).

Items §2.6 (m1 lock-side gap, no failure surfaced) and
§2.8 (pi-as-diagnostic-tool README recommendation) are
recorded-only — no follow-up commit needed in m4.

Two remaining post-merge driver tasks (intentionally
out of scope for the round-2 retro patch):
`docs(rafaello-plans-README): per-commit target/ dir
cleanup + pi-as-diagnostic for env/sandbox failures`
(§4.1 + §2.8), and an optional `docs(rafaello-decisions)`
row for the carveout-decomposition rule (`0a0e824`).

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
| `cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture` green on Linux inside devshell | ✅ **608 passed / 0 failed / 0 ignored** across 382 top-level `tests/*.rs` files (Cargo runs ~398 test targets including unit + doc tests), captured 2026-05-11 and re-confirmed on the round-2 branch (`/tmp/m4-acceptance.log`) |
| Demo-bar headline `rfl_chat_demo_bar_read_file.rs` green | ✅ `test rfl_chat_demo_bar_read_file ... ok` (`/tmp/m4-acceptance.log`); restored under the real Landlock sandbox by `0a0e824` (carveout decomposition fix) |
| **macOS CI green** (hard gate) | ✅ run `25655924846` captured 2026-05-11 against the RATIFIED commit `0cc1405` (`manual-validation.md` §4); both `test (ubuntu-latest)` and `test (macos-latest)` jobs of `rafaello.yml` passed |
| `cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture` green | ✅ captured 2026-05-11 (`/tmp/m4-build.log` — `Finished dev profile`) |
| `cargo doc --manifest-path rafaello/Cargo.toml --workspace --no-deps` warning-free | ✅ re-captured 2026-05-11 on round-2 branch after the bus.rs intra-doc-link fix (`639d7f0`); `/tmp/m4-doc.log` has zero `warning:` lines |
| `manual-validation.md` records interactive `rfl chat` demo + macOS CI URL | ✅ all six items captured or owner-accepted: §1-§3 + §5 captured on the dev host; §4/§6 macOS CI green captured 2026-05-11 as run `25655924846`; §5 interactive recording owner-accepted in lieu of capture (mechanical coverage via `rfl_chat_demo_bar_read_file`; recording deferred to m6 where `rfl init` lands) |
| Stream A §10 v1-summary banner patch | ✅ §2.1 — landed as `c222087` |
| `PublisherIdentity::Provider` Stream A schema additions | ✅ §2.2 — round-2 `9bd24e3` introduced the entry; round-3 `2e87e44` added the `topic_id` segment per pi-r2 B1; round-4 `41768a5` corrected the banner's live-API claims (`register_provider` signature + `RegisteredProvider` RAII type + `bindings.provider` lock shape + `Broker::new` `InvalidTopic` check location) per pi-r3 B2 |
| `decisions.md` row for `BusEvent.request_id` rollout | ✅ §2.3 — landed as `d51caba` (row 43) |
| `decisions.md` row for `Publisher::Provider` variant | ✅ §2.4 — round-2 `d51caba` ratified row 44; round-3 `fdac914` added `topic_id` to the `PublisherIdentity::Provider` shape and softened the rationale per pi-r2 B1 + N2; round-4 `63abf51` corrected the rationale to match the live `provider_active` vs `provider_id` distinction per pi-r3 B1 |
| m1 `check_lock_publish_topic` unknown-namespace gap | §2.6 — re-filed for m5+, no commit needed |
| Provider-side env-var documentation in `overview.md` §4.6 | ✅ §2.5 — landed as `63f6997` |
| `retrospective.md` written with anticipated drift addressed | ✅ this document (round 4; pi-review-1 closed in round 2, pi-review-2 closed in round 3, pi-review-3 closed in round 4 — see "Round-4 patch summary") |

---

**m4 ratified and closed 2026-05-11 by the owner.**
Pi-review-1 (5/5 blockers + 2/2 nits) closed in round 2;
pi-review-2 (2/2 blockers + 2/2 nits) closed in round 3;
pi-review-3 (3/3 blockers + 2/2 nits) closed in round 4;
**pi-review-4 (0/0 blockers + 2/2 polish nits) closed
by `190f5d7` and ratification declared**. §5 interactive
`rfl chat` recording owner-accepted in lieu of a screen
capture (mechanical coverage via the c27 headline test
`rfl_chat_demo_bar_read_file`; recording deferred to m6
where `rfl init` lands per the roadmap). The 71-commit
m4 cycle rebased and ff-merged onto `rafaello-v0.1` for
linear history (decisions row 33). **macOS CI green
captured 2026-05-11 as run `25655924846`** against
the RATIFIED commit; `manual-validation.md` §4 + §6
record the URL. All ratification gates met.
m5 inherits: the new `Provider` publisher class + envelope
machinery, the `subscribe_internal` primitive and
`ReemitRouter`, the `AgentLoop` + tool-dispatch wiring,
the `frontend.tui.user_message` ACL grant, the
`RFL_TUI_TEST_MESSAGE` env-hook, the two new plugin
fixtures (`rafaello-mockprovider` + `rafaello-readfile`),
the `rfl chat` four-level orchestration tree, and the
post-fix carveout decomposition that classifies children
by resource type.
