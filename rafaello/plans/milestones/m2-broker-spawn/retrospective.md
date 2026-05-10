# m2 — broker + locked plugin spawn — retrospective

> **Status:** revised round 2 on 2026-05-10 by the milestone
> driver after pi review-1, addressing all seven blocking
> findings + three non-blocking polish items. The `8ea4502^..1d68b5b`
> range carries **31 plan-row commits + one docs-only restructure
> commit (`d7c7705`)** = 32 commits total landed on
> `rafaello-v0.1` before the retrospective; scope (`scope.md`
> round 11, ratified) and commits (`commits.md` round 5,
> ratified). Worktree `/home/luiz/lab-wt/m2-retro-r2` on
> `agents/m2/retro-r2` off `rafaello-v0.1`. Per
> `plans/README.md` §"Patterns from prior milestones",
> retrospectives need adversarial review and m1 needed four
> rounds; another pi pass is expected after this revision.
>
> Companion: `manual-validation.md` (c31 evidence — 357 tests
> green, headline `supervisor_spawn_fixture_happy_path` runs in
> 2.26 s, three follow-ups F1–F3 surfaced).

This is the milestone-level review against `scope.md` and
`commits.md` per `plans/README.md` Phase 3. The five sections
match the questions the driver was asked to answer.

> **Numbering note.** `commits.md` enumerates plan rows c01–c31.
> The git log carries 31 implementation commits in 1:1
> correspondence (no bundling). One additional docs-only commit
> `d7c7705 docs(rafaello-m2): Phase-3 restructure — defer c22
> integration tests to c23, add driver-notes.md` lands between
> the c21 commit and the c22 commit; it is the `commits.md`
> + `driver-notes.md` patch that documents the c22 round-1
> restructure and is not a `commits.md` plan row. See §3.2 below.

---

## 1. Coverage

Every named **behaviour** in `scope.md` §"Demo bar" — both the
positive and the negative integration matrices — is covered
under `rafaello/crates/rafaello-core/tests/`, with the
file-name changes / merges enumerated in the reconciliation
table below (§"Scope-vs-landed file-name reconciliation"). The
on-disk listing contains the full broker (`broker_*.rs`),
supervisor (`supervisor_*.rs`, **32** files), fixture
(`fixture_*.rs`, **7** files), and crate-level (`error_*.rs`,
`topic_id_*.rs`, m1-carryover) suites. The c31 capture
aggregates **357 tests passed; 0 failed; 0 ignored** under
`--features test-fixture` (`manual-validation.md` §1).

### Scope-vs-landed file-name reconciliation

Three scope-named files do not match the landed file names
1:1; the behaviours are covered, but the files were renamed or
merged during implementation. Recording these explicitly so
the on-disk inventory can be cross-checked without surprise:

| `scope.md` test name | Landed file(s) | Reason |
|----------------------|----------------|--------|
| `bus_event_schema_round_trip.rs` | `bus_wire_types_schema.rs` | c06 — `BusEvent` derives are `Serialize`-only (one-way), so the round-trip framing was dropped in favour of schema/wire-type assertions on the serialised form (per pi-1 B1 of `commits.md` round 1). |
| `supervisor_lifecycle_drop_kills_child.rs` | `supervisor_drop_kills_managed_children.rs` | c26 — clearer name reflecting the actual `Drop for PluginSupervisor` reaper-handoff behaviour rather than a per-handle "lifecycle" framing. |
| `supervisor_spawn_reserved_env_helper_refused.rs` | folded into `supervisor_spawn_reserved_env_in_pass_refused.rs` + `supervisor_spawn_reserved_env_in_set_refused.rs` | c16 — the reserved-env coverage became table-driven across both `[env.pass]` and `[env.set]` matrices; `RFL_HELPER_FD` is one row of those tables rather than its own file. |

Plus one file-name disambiguation already documented below
(`supervisor_spawn_fixture_lifecycle.rs` at c21 vs the
canonical headline `supervisor_spawn_fixture_happy_path.rs` at
c30 — pi-2 B5).

### Positive matrix verification

The §"Demo bar" positive matrix is satisfied. Spot checks:

| scope.md test file | Landed in | Notes |
|--------------------|-----------|-------|
| `broker_acl_extraction.rs`, `broker_acl_tool_owner_resolves_routing.rs` | inherited from m1 (`f280498`) | m1's G1/G2 tests carry forward unchanged. |
| `broker_register_unregister.rs`, `broker_register_canonical_not_in_acl_rejected.rs`, `broker_register_duplicate_rejected.rs`, `broker_invalid_acl_rejected_at_construction.rs` | `db99316` (c07) | Registration positives + ACL revalidation. |
| `broker_publish_boot_event.rs` | `a64d4c7` (c08) | Explicit `publish_boot()` per pi-2 §1 / pi-6 non-blocking #2 (no auto-emit). |
| `broker_publish_round_trip.rs`, `broker_publishing_plugin_excluded_from_own_fanout.rs`, `broker_unsubscribed_plugin_does_not_receive.rs`, `broker_tool_result_not_fanned_out_to_other_plugins.rs` | `923c443` (c12) | Fan-out + publisher exclusion + §B7 result-routing protection. |
| `broker_publish_core_happy_path.rs` | `1471471` (c13) | `publish_core` positive. |
| `broker_handle_publish_after_unregister_returns_not_registered.rs` | `9cca869` (c09) | Live-registration check. |
| `broker_publish_rejected_event_fired_on_each_rejection_class.rs` | `1471471` (c13) | `core.lifecycle.publish_rejected` (§B9). |
| `supervisor_spawn_fixture_happy_path.rs` (canonical headline) | `4b415b1` (c30) | Two-fixture publisher + observer per scope §"Demo bar". |
| `supervisor_bus_publish_round_trip_two_plugins.rs`, `supervisor_peer_call_plugin_to_core.rs`, `supervisor_peer_call_core_to_plugin.rs`, `supervisor_taint_round_trip.rs`, `supervisor_private_state_dir_writable.rs` | `4b415b1` (c30) | c30 lands the canonical-five — five tests in one commit per pi-3 / round-4 inlining. |
| `supervisor_spawn_handle_clone_observes_same_outcome.rs` | `5378718` (c21) | Phase B step 18-20 + headline lifecycle test. |
| `supervisor_spawn_duplicate_canonical_refused.rs` | `e3b2e38` (c24) | Real duplicate-spawn refusal (pi-1 B5 deferred from c15). |
| `supervisor_shutdown_clean.rs`, `supervisor_shutdown_forced.rs` | `6673640` (c25) | Cooperative `shutdown(self)` + grace + SIGKILL paths. |
| `supervisor_drop_kills_managed_children.rs` | `e19249c` (c26) | `Drop for PluginSupervisor` reaper handoff. |
| `supervisor_env_pass_set_applied.rs`, `supervisor_env_set_overrides_pass.rs`, `supervisor_env_clear_strips_unrelated.rs`, `supervisor_env_application_compiles.rs` | `2ced41a` (c27) | Env behaviour matrix. |
| `supervisor_proxy_starts_and_env_injected.rs` | `eb7e13a` (c28) | Outpost proxy + env-injection end-to-end. |
| `supervisor_lockin_denies_outside_grant_read.rs`, `supervisor_lockin_denies_outside_grant_write.rs` | `3e55646` (c29) | Lockin denial proofs. |
| `fixture_publish_one_emits_event.rs`, `fixture_call_core_then_exit_completes.rs`, `fixture_dump_env_returns_allow_listed_keys.rs` | `ac57a95` (c23) | Three integration tests originally listed under c22; relocated to c23 by the 2026-05-09 restructure (§3.2). |

### Negative matrix verification

The §"Negative matrix" Phase A refusals all landed in c15/c16
under their canonical names (`supervisor_spawn_canonical_not_in_acl_refused`,
`supervisor_spawn_topic_id_mismatch_refused`,
`supervisor_spawn_relative_path_refused`,
`supervisor_spawn_relative_spawn_path_refused`,
`supervisor_spawn_control_chars_in_path_refused`,
`supervisor_spawn_entry_not_executable_refused`,
`supervisor_spawn_invalid_proxy_allow_hosts_refused`,
`supervisor_spawn_reserved_env_in_pass_refused`,
`supervisor_spawn_reserved_env_in_set_refused`,
`supervisor_spawn_provider_lock_refused`). Broker negatives
(`broker_publish_*_rejected.rs`, `broker_*_in_reply_to_*_rejected.rs`,
`broker_publish_extra_field_rejected.rs`) all landed across
c09–c13. Spot-check against `ls tests/` confirms presence for
every scope-named behaviour, with the three file-name
changes/merges enumerated in the reconciliation table above.

### Test renamed during Phase 3

`supervisor_spawn_fixture_lifecycle.rs` (c21) is the **non-canonical**
"three lifecycle assertions" test — explicitly disambiguated from
the canonical scope-named headline test
`supervisor_spawn_fixture_happy_path.rs` (c30) per pi-2 B5. Both
files land at their planned commits; the disambiguation is
already documented in `commits.md` lines 989–994 and is not a
post-hoc rename.

### Tests added beyond the matrix (examples)

The list below is illustrative, not exhaustive — other
non-matrix tests required by individual `commits.md`
acceptance lines or per-commit greenness include
`m2_error_surface_compiles.rs`, `bus_wire_types_schema.rs`,
`supervisor_spawn_starts_proxy_for_proxy_plan.rs`, and
`supervisor_spawn_skips_proxy_for_deny_plan.rs`. Examples:

- `supervisor_types_compile.rs` (c14), `fixture_modes_compile.rs`
  (c22) — build-only compile-surface assertions, the same
  pattern m1 used at c02 / c28.
- `fixture_binary_resolves.rs`, `fixture_binary_unknown_mode_exits_64.rs`,
  `fixture_responds_to_ready_then_holds_open.rs` (c20) —
  fixture-binary unit-style positives covering scaffold,
  unknown-mode exit code, and the `respond_peer_call` ready
  handshake.
- `harness_lock_builder_round_trip.rs` (c23) — round-trip on the
  `FixtureLockBuilder` helper itself, per c23 acceptance.

All extras are additive; none replace a scope-named test.

### Coverage verdict

**Real coverage loss, recorded explicitly:** c21 (`5378718`)
deleted **three** synthetic-stub unwind tests staged at c19:

- `supervisor_spawn_unwinds_after_register.rs` — post-register
  unwind window;
- `supervisor_spawn_post_register_reaps_child.rs` — Linux-only
  post-register reap observation;
- `supervisor_spawn_unwinds_after_socketpair.rs` — earlier
  Phase-B unwind window after socketpair / sandbox-builder
  setup but **before** `register_plugin` and child spawn,
  including the Linux fd-count return-to-baseline assertion.

All three covered synthetic "Phase B step 18+ not yet
implemented" failure paths that c21 removed when it finalised
steps 18–20. The canonical happy-path
(`supervisor_spawn_fixture_happy_path.rs`) and the duplicate
refusal (`supervisor_spawn_duplicate_canonical_refused.rs`)
both rely on a working spawn end-to-end and don't exercise any
unwind. **No fault-injection coverage of the Phase B unwind
windows remains in m2** — neither the post-register window
nor the pre-register socketpair / proxy / `tokio_command`
window. The proposed §5.1 m3 fault-injection follow-up only
injects after `register_plugin`, so it does **not** restore
the pre-register socketpair-window coverage on its own; a
separate pre-register injection point is filed alongside it.
Filed for m3 as §5.1 below.

Outside that one loss every scope-named positive and negative
test is implemented and passing; the c31 acceptance gate is
met (`manual-validation.md` §1: 357 tests / 0 failed under
`--features test-fixture`).

---

## 2. Drift against overview / decisions / stream RFCs

The drift items below were **anticipated** by `scope.md`
§"Acceptance summary" — eight bullets pre-named the items the
m2 retrospective was expected to address. This section catalogs
each, plus drift surfaced during Phase 3 review of the landed
code. Categories follow m1's three buckets (RFC body patches,
overview/decisions/glossary text, recorded-only RFC drift).

Per `plans/README.md` §"Authoring conventions", stream RFCs are
not retroactively rewritten; m1's banner-based reconciliation
is the documented exception and the precedent for m2 patches.
**This retrospective records each drift item with the canonical
fix; the actual `decisions.md` rows / overview patches / Stream
A banners land as separate follow-up commits on this branch
before milestone close**, mirroring m1's "follow-up commits on
this branch" pattern (m1 retrospective §"Follow-up commits").

### 2.1 Provider-rejection staging (anticipated)

m2's `SpawnError::InvalidPlan { reason: ProviderNotInM2 {
provider_id } }` refuses lock entries with `bindings.provider
= true` (`supervisor_spawn_provider_lock_refused.rs`). The
`provider_id` payload carries the offending lock entry's
provider id from the compiled ACL/provider metadata (c16
acceptance). m4 owns the provider plugin path
(`provider.<id>.*` publish authority,
`core.session.tool_request` re-emission per `decisions.md` row
6). Until then, m2 ships the refusal so the demo bar is
unambiguous: m2 spawns plugins, not providers.

**Canonical fix.** Append `decisions.md` row 39:

> 39 | 2026-05-10 | m2 supervisor refuses lock entries with
> `bindings.provider = true` (`SpawnError::InvalidPlan {
> reason: ProviderNotInM2 { provider_id } }`, with the
> provider id surfaced in the error). m4 removes this refusal
> and implements provider routing per row 6. | Provider plugins
> have distinct publish authority + tool re-emission semantics
> that m4 owns end-to-end; refusing them at m2 keeps the demo
> bar tight and prevents partial implementations from leaking
> into integration tests. | ratified | — |

(pi-1 §10, §132, §437.)

### 2.2 `request_id` omission in `BusEvent` (anticipated)

`overview.md` §4.5 enumerates `request_id` as part of the bus
envelope; m2's `BusEvent` shape (c06, `bus.rs`) omits it because
m4's tool-dispatch flow is the only consumer. The `BusEvent`
+ `bus.event` outbound method are the live wire shape today.

**Canonical fix.** Patch `overview.md` §4.5 with a v1 staging
note:

> *m2 staging note (2026-05-10):* `request_id` is reserved but
> not populated by m2's broker. The field is consumed by m4's
> tool-dispatch flow (`core.session.tool_request` →
> `core.session.tool_result`); plugin-publish events on
> `plugin.<topic-id>.*` and `core.lifecycle.*` do not carry it.
> m4 retrospective re-validates the envelope.

(pi-1 §41, §145, §438.)

### 2.3 `core.lifecycle.publish_rejected` + `core.lifecycle.boot` schemas (anticipated)

m2 introduces both events. The `core.lifecycle.boot` event is
specified at scope §B1 (the `publish_boot` bullet) with the
acceptance-summary callout for its schema; the
`core.lifecycle.publish_rejected` family is specified at scope
§B9, which also discusses the boot schema in the rejected-
event family context. Cite both §B1 and §B9 when referencing
the boot event's location (c08 + c13). The security
RFC has no entries for them. Per `decisions.md` row 23 (Stream
A owns payload schemas), this is Stream A drift — analogous to
m1's helper / external-attach drift at security RFC §7.4.1 +
§5.7.

**Canonical fix.** Add a new §"`core.lifecycle.*` events"
section to `streams/a-security/rfc-security-model.md`
documenting the two payload schemas exactly as scope §B9
specifies, OR — preferred per the m1 banner precedent — append
a v1-status banner pointing at scope §B9. The banner form
respects the README's "RFCs are historical artefacts" rule
without rewriting body sections.

This is grouped with §2.4 below into one Stream A patch
commit.

### 2.4 `bus.event` outbound method + `BusEvent` shape + `PublisherIdentity` enum (anticipated)

m2 fixes:

- the outbound notification method as `bus.event` (c06,
  `bus.rs`),
- the `BusEvent` JSON shape per scope §B8,
- the `PublisherIdentity` enum — **m2's live wire schema is
  exactly two variants**: `Core` and
  `Plugin { canonical: String, topic_id: String }` (per the
  landed `bus.rs:48-51`). `Provider` and `Frontend` are
  reserved/future variants for m3–m5 and are **not** part of
  the m2 wire schema; scope §B8 already lists them only as
  future-commented variants.

None of the live m2 variants appear in Stream A's RFC body.
Same Stream A drift bucket as §2.3.

**Canonical fix.** Same Stream A banner / follow-up section as
§2.3. The single banner can carry §2.3 + §2.4 together, and
must document only the live m2 variants of `PublisherIdentity`
(`Core` + `Plugin { canonical, topic_id }`); future
provider/frontend variants are out of scope for the m2 banner:

> `docs(rafaello-stream-a): banner — m2 wire schemas
> (bus.event, BusEvent, PublisherIdentity { Core, Plugin },
> core.lifecycle.*)`

### 2.5 m1 reserved-env list extended (anticipated)

c04 (`b6646ea`) extends m1's reserved-env constant set with
`RFL_HELPER_FD`, `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`,
`RFL_PRIVATE_STATE_DIR` — small back-reach to m1, scope §"In
scope" group 1. Touches `rafaello-core/src/manifest/...`'s
reserved-env validation and is exercised by m2's
`supervisor_spawn_reserved_env_in_pass_refused` /
`...in_set_refused` tests.

**Canonical fix.** Append `decisions.md` row 40:

> 40 | 2026-05-10 | The reserved-env list (m1 manifest
> validation) extends to include `RFL_HELPER_FD`,
> `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`,
> `RFL_PRIVATE_STATE_DIR` for m2. `RFL_HELPER_FD` is reserved
> for the deferred helper-plugin path (row 26 v2 deferral);
> the others are populated by the m2 supervisor at spawn time
> and must not be set in `[env.pass]` / `[env.set]`. | Bus
> authority + supervisor-injected paths are forge-proof only
> if plugins cannot pre-populate them. The list grows with
> each milestone that adds a new reserved name; capturing the
> m2 additions explicitly. | ratified | refines #13 |

(pi-1 §23, §270.)

### 2.6 Lock-correspondence is API-level only (anticipated)

`scope.md` §"Goal" already records that `CompiledPlugin`'s
public fields permit hand-mutation: a caller could synthesise a
plan with mismatched canonical / topic-id / paths. m2's Phase A
spot-checks the cases that would otherwise crash the lockin
builder (canonical-not-in-ACL, topic-id-mismatch, relative
paths, control-chars, non-executable entry, invalid proxy
allow-hosts, reserved env names, provider lock). It does not
prove forge-resistance.

**Verification.** Re-reading the landed
`rafaello/crates/rafaello-core/src/{compile.rs,supervisor.rs}`
confirms `CompiledPlugin` is `pub struct` with `pub` fields —
the deviation as scope describes it still holds. Tightening to
an opaque/validated plan type is a v2 nice-to-have, not a v1
fix. Recorded as no-action for v1; logged here so the m4 / v2
driver doesn't rediscover it.

### 2.7 Result-routing protection — m4 handover (anticipated)

c12 (`923c443`) wires §B7: the broker does not fan out
`plugin.<id>.tool_result` / `rpc_reply` envelopes. The current
implementation drops them with a `tracing::debug!`. m4 replaces
that no-op with the canonical re-emission path
(`core.session.tool_result` carrying the inherited
`in_reply_to` per `decisions.md` row 8).

**Canonical fix.** Recorded only — the m4 driver inherits
this section verbatim as the §B7 handover note. No
overview/decisions/RFC patch required (the drop-and-trace
behaviour is already documented in scope §B7). m4 retrospective
will close the loop.

`broker_tool_result_not_fanned_out_to_other_plugins.rs` (c12)
freezes the current behaviour as a regression baseline — m4
must update or replace this test when re-emission lands.

### 2.8 m1 publishes-grant unknown-namespace gap — m3-or-m4 follow-up (anticipated)

m1's manifest validation may permit unknown top-level
namespaces in `publishes` grants (pi-1 §254, §255, §409). m2's
broker rejects them at runtime
(`broker_publish_unknown_namespace_rejected.rs`); the parse-
and validation-time mirror in m1 was never tightened.

**Canonical fix.** Recorded as a m3-or-m4 follow-up patch to
m1; **not** a m2 in-scope item per scope §"Acceptance summary".
The m3 driver (TUI / loop) inherits this as a known gap from
m2; a parse-time tightening commit lands at m3 or m4 as a small
back-reach.

### 2.9 SandboxBuild rename + outpost policy/enforcement framing (Stream A drift, recorded)

Pi's runtime-extensibility verification round (round 10 of
scope) renamed `SpawnError::Lockin` → `SpawnError::SandboxBuild`
and added two framing notes to the scope: peer admission as the
load-bearing extensibility seam (not "runtime backend"), and
`outpost::NetworkPolicy` as the durable policy vocabulary with
`outpost-proxy` only the lockin enforcement backend. These are
already pinned in the scope round-10 ratified text and the
landed `error.rs` carries the `SandboxBuild` name. No further
patch needed; recorded for m3+ readers who may hit lockin
implementation details and want the framing.

### 2.10 Manual-validation cargo-doc warning (carried into §5)

c31 capture surfaced one `rustdoc::private_intra_doc_links`
warning at `crates/rafaello-core/src/supervisor.rs:108`
(public `SpawnHandle` doc comment links private
`ManagedSpawn`). Direct analogue of m1 F1.
`manual-validation.md` §"Follow-ups" F3. One-line fix lands as
part of the retrospective drift-fix follow-up commits. Tracked
in §5.2 below.

### 2.11 Manual-validation tracing setup (carried into §5)

`supervisor_spawn_fixture_happy_path` does not initialise
`tracing-subscriber`, so `RUST_LOG=rafaello_core=debug`
produces no output from the test's own emissions. F1 in
`manual-validation.md`. One-liner add (`tracing_test::traced_test`).
Tracked in §5.3 below.

### 2.12 Manual-validation `bus/` + `supervisor/` submodule wording (carried into §5)

scope §"Manual validation" / commits c06+c07+c14 sketch
`bus/publish_msg.rs` and `supervisor/lifecycle.rs` submodules;
the landed code is single-file `bus.rs` (443 lines) and
`supervisor.rs` (925 lines). Cosmetic scope drift; F2 in
`manual-validation.md`. Tracked in §5.4 below.

### 2.13 Verdict

**Eight drift items** — six action-required (anticipated)
plus two surfaced during c31 capture (pi-2 non-blocking #2 —
items 7-8 are recorded-only / deferred, not patched in m2).
Per scope §"Acceptance summary"'s explicit assignment, each
action-required item lands as a follow-up commit on this
branch before milestone close:

1. **Append `decisions.md` row 39** — provider-rejection
   staging (§2.1).
2. **Append `decisions.md` row 40** — reserved-env extension
   (§2.5).
3. **Patch `overview.md` §4.5** — `request_id` v1 staging
   note (§2.2).
4. **Stream A v1-status banner / new section** — lifecycle
   event schemas + `bus.event` + `BusEvent` +
   `PublisherIdentity` (§2.3 + §2.4).
5. **Source fix `supervisor.rs:108`** — cargo-doc
   `private_intra_doc_links` warning (§2.10 / F3).
6. **Test fix `supervisor_spawn_fixture_happy_path.rs`** —
   tracing-subscriber init (§2.11 / F1).
7. **Scope wording sweep** — `bus/`, `supervisor/` submodule
   references (§2.12 / F2).
8. **Recorded only** — §2.6 (lock-correspondence v2
   nice-to-have), §2.7 (m4 handover), §2.8 (m3/m4 m1 patch),
   §2.9 (SandboxBuild already in code).

The retrospective does **not** ratify until items 1–7 land or
are explicitly waived. m1 set this precedent (m1 retrospective
§2.7 verdict).

---

## 3. Slipped or cut

### 3.1 No item from scope's lists slipped or was cut

Every entry in scope's `W` / `B` / `SP` / `H` / `F` / `I` / `H6`
/ `E` lists landed. Every named test (positive + negative
matrices) is implemented except the three unwind tests at
§3.3 below, which were **deleted with cause**, not slipped.

### 3.2 c22 → c23 mid-Phase-3 restructure (relocation, not slip)

`commits.md` c22 round-1 originally bundled four things:

1. fixture-mode dispatch additions (publish-and-exit, observer,
   call_core_then_exit, extra peer-call methods);
2. `tests/fixture_publish_one_emits_event.rs`;
3. `tests/fixture_call_core_then_exit_completes.rs`;
4. `tests/fixture_dump_env_returns_allow_listed_keys.rs`.

The c22 round-1 agent hung for >1 hour on item 4 because tests
2–4 call methods like `core.fixture.dump_env` and
`core.fixture.observed` that the production
`BusPublishService` (c19) doesn't route — they require the
`with_extra_service` extra-service plumbing the c23 harness
ships. The c22 worktree had no harness yet and the agent's
attempts to thread it ahead of c23 produced 21 dangling fixture
processes before the driver killed the round.

**Resolution** (commit `d7c7705` — the docs-only Phase-3
restructure commit between c21 and c22): c22 reduced to *only*
the fixture-mode dispatch + a build-only `fixture_modes_compile.rs`
assertion. The three integration tests moved to c23
alongside the harness they depend on. Tests 2–4 land in
`ac57a95` (c23) per §1's trace table.

This is a relocation, not a slip — every test is in the suite
at the same point in `rafaello-v0.1`'s history; only the
plan-row → git-commit boundary moved. Fully captured in
`driver-notes.md` §"c22 restructure 2026-05-09" so the m3
driver doesn't re-discover it.

**Lesson promoted to §4.1 below.**

### 3.3 Three unwind tests deleted by c21 with cause

`commits.md` c19 acceptance lines list three c19-staged
synthetic-stub unwind tests, all deleted by c21 (`5378718`):

- `tests/supervisor_spawn_unwinds_after_register.rs` —
  asserts spawn returns the synthetic "Phase B step 18+ not
  yet implemented" error after socketpair + proxy + child
  spawn + register all succeed; broker still has canonical in
  ACL but no live registration; `in_flight` cleared.
- `tests/supervisor_spawn_post_register_reaps_child.rs`
  (Linux-only) — asserts the post-register reap fires when
  c19's stub returns `Err`, observed via a `TestHooks`
  `last_reaped_pid` counter; `/proc/<pid>/status` returns
  `ENOENT`.
- `tests/supervisor_spawn_unwinds_after_socketpair.rs` —
  asserts the **earlier** Phase-B unwind window after
  socketpair / sandbox-builder setup but **before**
  `register_plugin` and child spawn, including the Linux
  fd-count return-to-baseline assertion. This is a different
  failure window from the post-register pair above.

All three tests exercised synthetic stub failure paths baked
into c19. **c21 finalises Phase B steps 18–20** — the stub is
removed and replaced with the real `Server::serve` /
`SpawnHandle` return. Once c21 lands, the synthetic `Err` no
longer exists; the tests have nothing to assert against. The
c21 agent (per the per-commit prompt) deleted all three files
rather than rewriting them.

**This is a real coverage loss spanning two distinct windows:**

- the **post-register** window covered by
  `unwinds_after_register` + `post_register_reaps_child` —
  spawn body fails after `register_plugin` succeeded; verify
  broker rolled back, child reaped, `in_flight` cleared, no
  fd / proxy / private-state leaks. Real-world failures here
  can come from:
  - transport setup faults after broker register (e.g.
    `Server` construction or transport binding errors);
  - `peer.call` / supervisor extra-service installation errors
    before the spawn returns `Ok`;
  - panics / cancellation in the spawn body's tail.
- the **pre-register / post-socketpair** window covered by
  `unwinds_after_socketpair` — sandbox builder / socketpair /
  proxy / `tokio_command` setup succeeded but a downstream
  step before `register_plugin` fails; verify fd counts return
  to baseline and no proxy / private-state leaks. Real-world
  failures here can come from socketpair errors,
  outpost-proxy bind failures, or `tokio_command` spawn
  errors.

Neither window has a successor in m2's suite.

**For m3 fault-injection coverage.** The right successor isn't
"resurrect the synthetic stub" — it's a `TestHooks::inject_fault`
mechanism with **two** inject points (one pre-register, one
post-register) so the suite can re-cover both windows without
modifying production code. The post-register injection point
restores the `unwinds_after_register` /
`post_register_reaps_child` window; the pre-register
injection point (after socketpair / proxy / `tokio_command`,
before `register_plugin`) restores the
`unwinds_after_socketpair` window. Filed as §5.1.

### 3.4 No m2a / m2b split was triggered

`scope.md` §"Internal split" + `commits.md` §"m2a / m2b
checkpoint" flagged a possible split after c19 (broker complete
+ Phase B mid-spawn). The driver landed c19 → c31 cleanly on
top of c01–c18 without forcing a split, despite the c22
restructure mid-stream. Default ("ship m2 as one milestone")
prevailed.

### 3.5 No extra-scope features added during implementation

Every test and code path traces back to a row in `scope.md` or
to a `commits.md` per-commit acceptance line. The extras named
in §1 ("tests added beyond the matrix") are all
`commits.md`-ratified or per-commit-greenness forced — none
represent feature creep.

---

## 4. Process notes for the next milestone driver

These are sharp edges m2 hit that aren't already in
`plans/README.md` §"Recurring operational gotchas" or §"Patterns
from prior milestones". The m3 driver should plan around them.

### 4.1 Tests that need harness plumbing must land in or after the harness commit

m2 violated this once (c22 round-1) and paid for it with a
>1-hour agent thrash + 21 dangling fixture processes. The
broader rule generalises:

> When `commits.md` schedules an integration test before the
> commit that ships the supporting harness/wiring, the
> per-commit agent will either invent a one-off harness
> (scope creep) or thrash on the missing surface
> (wall-clock loss). Schedule the test in or after the
> harness commit.

The c22 round-1 prompt cited "the c21 supervisor surface is
enough" — it wasn't, because c21 only exposes the production
`BusPublishService` route. Tests that need to call
`core.fixture.<extra_method>` need
`with_extra_service` + `ExtraServiceFactory`, which only c23
ships.

**Recommendation.** When drafting `commits.md`, stage every
integration test against the latest commit whose surface the
test exercises — not the latest commit whose code the test
exercises. The two are different when extra-service plumbing
is the surface.

**Also.** Mid-Phase-3 restructures **are OK** but require a
`driver-notes.md` entry recording the restructure (commit
`d7c7705` is m2's example) so a future bisect or audit can
reconstruct the change. m1 had no such entry because m1 had no
mid-Phase-3 restructure; m2 sets the precedent for capturing
them.

**Generalised lesson — synthetic stub tests.** c19 staged
three unwind tests against a synthetic stub-failure path that
c21 was scheduled to remove (see §3.3); c21 deleted the tests
rather than rewriting them, leaving the unwind windows
uncovered. The lesson generalises beyond c22/c23: **any
synthetic-stub test must have a planned successor (a
fault-injection point or equivalent permanent assertion
mechanism scheduled at the commit that removes the stub) or
an explicit deletion rationale recorded in `commits.md`
acceptance lines.** Without that, the stub deletion silently
costs coverage. Add this requirement to the m3 commit-prompt
template.

### 4.2 Pi-as-diagnostic-tool worked beautifully on the c23 publish_one hang

`driver-notes.md` §"c23 publish_one hang diagnosis 2026-05-10"
captures the symptom: the c23 agent had been thrashing for
>1 hour on `tests/fixture_publish_one_emits_event.rs` chasing
broker / subscription races. The driver paused the agent and
spawned a pi session pointed at the same code + the test
stderr. Pi root-caused it in <30 minutes:

> The publisher fixture reaches `bus.publish` then tries to
> use the flush ack as
> `client.call("core.fixture.after_publish", Value::Null)`.
> The fittings wire decoder rejects any present `params` that
> is not an object or array, so the request is `InvalidRequest`
> (`field 'params' must be an object or array when present`).
> That makes the publisher panic after sending `bus.publish`,
> which sends agents chasing broker/subscription races.

Fix: send object params (`json!({})`) for the flush ack.

**Pattern worth promoting to `plans/README.md`.** When a
per-commit agent has been thrashing on the same symptom for
>30–60 minutes with no narrowing, the right move is **pi as
read-only diagnostic** — pi reads the same files claude has but
isn't load-bearing on the trace state, so it judges the call
graph fresh. Two specific signals that pi is the right next
step:

- the agent's hypotheses are getting **wider** rather than
  narrower (claude proposes "maybe broker fan-out", "maybe
  subscription timing", "maybe transport buffering" in
  succession with no convergence);
- the agent is reaching for **timeout extension** rather than
  **failure simplification** (the m2 instinct was "extend the
  cargo timeout to 90 s" — almost always wrong; the right
  move is a smaller failing reproducer).

Recommended: add this to `plans/README.md` §"Recurring
operational gotchas" as "**Pi-as-diagnostic when a per-commit
agent stalls.**" Worth doing as a follow-up commit alongside
the §2 drift fixes.

### 4.3 Agent instinct to extend timeouts vs the bounded-wait discipline

scope §H5 + pi review-5 §1 enshrined a bounded-wait discipline:
every test's wait has a ≤5–10 s expected upper bound; longer
waits indicate a real problem, not a slow test. The c23 agent
violated this twice (extending the per-test timeout, increasing
the overall `cargo test` timeout). Both extensions delayed the
diagnosis without producing signal.

**Recommendation.** Per-commit prompts for tests that involve
process spawning should restate: "If a wait exceeds ~10 s the
test is wrong, not slow — escalate to the driver, do not
extend the timeout." Add this language to the m3 commit-prompt
template.

### 4.4 Fixture process leaks on test panic

The fixture's `respond_peer_call` mode sleeps until SIGTERM and
relies on the `SpawnHandle::Drop` (c26) for cleanup. **`Drop`
fires for in-test instances but not when the test panics
before the `SpawnHandle` exists** — e.g. during the spawn body,
in c20-style direct-`exec` tests, or under a `cargo test`
process kill (`-9` from the outer timeout). c20-vintage and
c22-round-1-vintage fixture leaks both reached the host process
table.

m2 has no in-process supervisor for runaway test fixtures; the
driver cleaned them up manually (`pkill` filtered by worktree
path).

**Recommendation.** Two options for m3+:

1. **Driver-side reaper.** Driver loops `pgrep -f
   '/m<N>-c<NN>/.*rfl-bus-fixture'` after each per-commit
   tear-down; any survivor is killed and logged. Cheap;
   doesn't change the codebase.
2. **Fixture self-timeout.** Fixture binary takes
   `RFL_FIXTURE_MAX_LIFETIME` (env-var, seconds) and exits 0
   after that even without SIGTERM. m4 territory if the fix
   ships in code rather than tooling.

Neither is m2's fix to land — recorded so m3 knows the trade.

### 4.5 `Cargo.lock` dirty in `/home/luiz/lab` silently aborts ff-merges

`driver-notes.md` §"Driver-side gotchas hit so far" captures
this concretely. Symptom: `git worktree add` shows the OLD
`rafaello-v0.1` tip after the agent's commit "merged"; the
ff-merge in the main checkout silently no-ops because the
working tree is dirty. Fix: stash `Cargo.lock` before
ff-merging.

**Recommendation: promote to `plans/README.md` §"Recurring
operational gotchas".** This is generic across milestones, not
m2-specific.

### 4.6 `.pre-commit-config.yaml` symlink missing in worktrees outside `/home/luiz/lab`

`driver-notes.md` flags this; `plans/README.md` §"Recurring
operational gotchas" already documents the orchestrator-side
workaround (`PREK_ALLOW_NO_CONFIG=1`) and the retrospective-
worktree workaround. m2's per-commit pattern symlinks the
config from the nix store at worktree-creation time
(`driver-notes.md` step 2), which is the cleaner fix for
agents running under `claude --dangerously-skip-permissions`.

**Recommendation.** Update `plans/README.md` §"Recurring
operational gotchas" to record the symlink-at-worktree-create
form as the preferred fix for agent worktrees, with
`PREK_ALLOW_NO_CONFIG=1` retained for orchestrator-side
commits. This is m2's contribution to the gotcha list.

### 4.7 Per-commit-prompt inlining held up across the milestone

m1 retrospective §4.2 asked: "always paste the full commit row
text + acceptance bullets into the per-commit prompt". m2's
driver did this for all 31 commits. **Result:** zero
plan-row-bundling incidents (vs m1's c31+c32 bundle), zero
implicit-granularity drift. The c22 restructure was an
agent-stalled-on-missing-surface issue, not a prompt-shape
issue. The m1 lesson is now well-validated.

### 4.8 Five rounds of pi review on `commits.md`, six on scope.md (per m1's "expect five" estimate)

`commits.md` ratified at round 5; scope at round 11 (6 pi
review rounds + 4 owner-driven polish rounds). m1 retrospective
§4.1 predicted "expect five rounds on commits.md" — m2 hit
exactly that. Scope took longer because runtime-extensibility
discussion was inserted mid-stream (round 10) per the
discussion document. Both numbers are now well-attested.

---

## 5. Known issues to track

### 5.1 Phase B unwind-window coverage gap (NEW — proposed m3 follow-up)

The three unwind tests deleted by c21 (§3.3) leave m2 without
fault-injection coverage of two distinct spawn-body windows:

- the **post-register** window, between
  `broker.register_plugin(...)` succeeding and `Ok(handle)`
  returning. Real-world failures there (transport setup,
  extra-service installation, panic / cancellation in the spawn
  tail) would hit the `Drop`-time SIGKILL handoff (c26) rather
  than the cooperative cleanup, but the cooperative path itself
  is unverified.
- the **pre-register / post-socketpair** window, between
  socketpair / proxy / `tokio_command` setup succeeding and
  `register_plugin` being called. Real-world failures there
  (proxy bind errors, command spawn errors, sandbox builder
  errors after fd allocation) need fd-count and proxy-cleanup
  verification with no successor in m2's suite.

**Proposed m3 fix.** Add `TestHooks::inject_fault` with **two**
one-shot inject points — one pre-register (after socketpair /
proxy / `tokio_command` setup, before `register_plugin`) and
one post-register (between `register_plugin` and
`Ok(handle)`). On hit, the supervisor returns
`SpawnError::SandboxBuild { source }` and triggers the unwind
path. Re-add `supervisor_spawn_unwinds_after_register.rs`,
`supervisor_spawn_post_register_reaps_child.rs`, and
`supervisor_spawn_unwinds_after_socketpair.rs` against the
fault-injection mechanism. **Proposed** (not yet
owner-blessed) because m3 is the TUI / loop milestone and
supervisor fault-injection is in its neighbourhood; owner
sign-off pending m3 scoping.

This is the single largest known coverage gap in m2.

### 5.2 ✅ to-resolve: `cargo doc` `private_intra_doc_links` warning

`crates/rafaello-core/src/supervisor.rs:108` —
`SpawnHandle` doc comment links private `ManagedSpawn`. Direct
analogue of m1 F1. One-line fix lands as part of the §2
follow-up commit batch.

### 5.3 ✅ to-resolve: `tracing-subscriber` not initialised in headline test

`supervisor_spawn_fixture_happy_path.rs` doesn't set up
`tracing-subscriber`, so `RUST_LOG=rafaello_core=debug` is a
no-op for the test's own emissions. F1 in `manual-validation.md`.
One-liner fix in the §2 batch.

### 5.4 ✅ to-resolve: scope wording on `bus/` + `supervisor/` submodule layout

scope §"Manual validation" + commits c06/c07/c14 sketch
`bus/publish_msg.rs` and `supervisor/lifecycle.rs`; landed
shape is single-file `bus.rs` (443 lines) + `supervisor.rs`
(925 lines). Cosmetic; F2 in `manual-validation.md`. Reword
in the §2 batch.

### 5.5 No flakes observed in the recorded run

The c31 manual-validation capture records **one aggregated
run** of the 357-test suite under `--features test-fixture`
with 0 failures (`manual-validation.md` §1). Flake re-runs
(repeated full-suite passes) were **not** done as part of
this milestone; the supervisor surface is largely
deterministic by construction
(`supervisor_drop_kills_managed_children`,
`supervisor_shutdown_*`, and `supervisor_spawn_*_refused`
assert on counters / error variants, not on timing windows),
but a flake budget remains a future-validation item if a
cross-platform CI matrix surfaces one.

### 5.6 No clippy warnings suppressed by agents

`git log -p 8ea4502^..1d68b5b -- rafaello/ | grep -E
'#\[allow\(clippy'` matches zero new sites across the m2
implementation range. The pre-commit hook sequence (rustfmt +
clippy + test) gates every commit.

### 5.7 ✅ Resolved: macOS CI green after one round of cross-platform fixes

Run: https://github.com/luizribeiro/lab/actions/runs/25623373610
on commit `7db9da8`. Both `test (ubuntu-latest)` and
`test (macos-latest)` jobs ✅ green.

**Round-1 push (`7b0daf4`) failed** with two CI-environment
issues the local devshell didn't expose:

1. **macOS:** `error[E0599]: SOCK_CLOEXEC found for SockFlag` —
   nix's SockFlag has SOCK_CLOEXEC only on Linux. Fix:
   cfg-gated CLOEXEC fallback using
   `nix::fcntl::fcntl(F_SETFD(FD_CLOEXEC))` after socketpair
   for non-Linux targets.
2. **Linux (CI):** `Linux sandbox requires syd but could not
   find it.` The `.#rafaello` devshell (used by CI) didn't
   export `LOCKIN_SYD_PATH` even though the local devshell
   did via the `lockin/nix/devenv.nix` overlay. Fix: mirror
   the `LOCKIN_SYD_PATH` export in `rafaello/nix/devenv.nix`.

Both fixes landed as `7db9da8` (single commit, since they
were both blocking the green CI run). `manual-validation.md`
§"macOS CI" captures the URL + result table.

**Lesson for m3+:** local `nix develop --impure` may pick up
a devshell that aggregates more than just `.#rafaello`'s own
exports. CI explicitly enters `.#rafaello`, exposing missing
exports. m2 added the `--features test-fixture` invocation
to the workflow during this round (per `commits.md` Risks
#1) — without it, `env!("CARGO_BIN_EXE_rfl-bus-fixture")`
would have errored before any test ran. m3 inherits the
workflow shape.

---

## Follow-up commits on this branch

Per pi review-1 of m1's retrospective, drift fixes land
**before** the retrospective ratifies. Planned for this branch:

1. ✅ `docs(rafaello-decisions): rows 39 (provider rejection in
   m2) + 40 (reserved-env extension)` — `2d49215`. §2.1 + §2.5.
2. ✅ `docs(rafaello-overview): §4.5 v1 staging — request_id is
   m4, m2 BusEvent adds publisher` — `73d83f9`. §2.2.
3. ✅ `docs(rafaello-stream-a): banner — m2 wire schemas
   (bus.publish, bus.event, BusEvent, PublisherIdentity,
   core.lifecycle.*)` — `7979f54`. §2.3 + §2.4.
4. ✅ `fix(rafaello-core): cargo doc — disambiguate SpawnHandle
   intra-doc link` — `08cc458`. §2.10 / §5.2 / F3.
5. ✅ `test(rafaello-core): init tracing-subscriber in
   supervisor_spawn_fixture_happy_path` — `cae8601`. §2.11 /
   §5.3 / F1.
6. ✅ `docs(rafaello-m2): scope wording sweep — bus/ +
   supervisor/ submodule references (single-file landed)` —
   `a350380`. §2.12 / §5.4 / F2.
7. ✅ `docs(rafaello-plans): promote pi-as-diagnostic +
   Cargo.lock + prek-symlink gotchas to README` — `cd00a75`.
   §4.2 + §4.5 + §4.6.

Items 1–6 are scope-§"Acceptance summary"-mandated. Item 7 is
optional polish; the m3 driver inherits the lessons from this
file regardless.

Pending pi adversarial review of this retrospective (per
`plans/README.md` §"Patterns from prior milestones": m1 needed
four rounds, plan calendar for at least two).

---

## Acceptance summary check

`scope.md` §"Acceptance summary" requires:

Legend: ✅ = both recorded **and** the underlying patch has
landed; 📝 = recorded in this retrospective with the canonical
fix specified, but the follow-up code/docs patch is still
pending; ⏳ = recorded and explicitly pending external action
(CI run, owner sign-off). Items in the 📝 / ⏳ states are
**not** ratified by this retrospective alone — they ratify
when the follow-up patch lands or the action completes.

- ✅ Every named **behaviour** in the positive + negative
  matrices is covered (pi-2 non-blocking #1 — exact file
  names diverge per §1's reconciliation table); 357
  `rafaello-core` tests passing under `--features
  test-fixture` on Linux per `manual-validation.md` §1.
  Three unwind tests (c19) deleted with cause by c21;
  m3 follow-up §5.1.
- ✅ `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture`
  green on Linux (`manual-validation.md` §1).
- ✅ macOS CI green (§5.7 — run 25623373610 on `7db9da8`,
  both Linux + macOS jobs success after one round of CI-only
  cross-platform fixes).
- ✅ `nix develop --impure --command cargo build --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture
  --bin rfl-bus-fixture` green (`manual-validation.md` §6).
- ✅ `nix develop --impure --command cargo doc --manifest-path
  rafaello/Cargo.toml -p rafaello-core --no-deps` warning-free
  (§2.10 / §5.2; fix landed in `08cc458`).
- ✅ `manual-validation.md` records the items in the manual-
  validation list (`1d68b5b` c31).
- ✅ `retrospective.md` records the eight anticipated drift
  items with their canonical fixes (this file, §2.1–§2.8);
  drift-fix commits 1–7 in §"Follow-up commits on this branch"
  all landed.
- ✅ Stream A schema additions — banner landed at `7979f54`
  (§2.3 + §2.4).
- ✅ Reserved-env list update + decisions row — landed
  alongside provider rejection in `2d49215` (§2.5 → row 40).
- ✅ Provider-rejection staging — `decisions.md` row 39 landed
  in `2d49215` (§2.1).
- ✅ `request_id` v1 staging note — `overview.md` §4.5 banner
  landed in `73d83f9` (§2.2).
- ✅ Lock-correspondence claim — recorded; no v1 patch
  required (§2.6).
- ✅ Result-routing protection m4 handover — recorded only;
  m4 owns the closure (§2.7).
- ✅ m1 publishes-grant unknown-namespace gap — recorded as
  m3-or-m4 follow-up (§2.8); m2 ratification does not depend
  on it.

m2 is **complete pending owner ratification.** The core
deliverable (broker + locked plugin spawn fixture) landed:
357 tests green, headline `supervisor_spawn_fixture_happy_path`
running in 2.26 s under real lockin + outpost-proxy with two
cooperating fixture plugins. Pi review converged in two
rounds (round 1 found 7 blocking; round 2 verdict: "no new
blocking findings"). All seven §"Follow-up commits on this
branch" items landed 2026-05-10. macOS CI green on
`7db9da8` after one round of cross-platform fixes (§5.7).
**Owner ratification is the only remaining gate.**
