# m3 — sessions, local-spawned TUI, built-in rendering — retrospective

> **Status:** revised round 3 on 2026-05-10 by the milestone
> driver after `retro-pi-review-2.md` (5 blockers + 3
> non-blocking polish). Round 2 had closed the round-1
> blockers but introduced four new factual errors
> (inventory count, c14/c15/c18 row mismatches,
> tui_subscribes split reconciliation, macOS Linux-only
> exemption framing) plus over-greened the acceptance
> table. Round 3 corrects each. Worktree `/home/luiz/lab` directly
> on `rafaello-v0.1` (m3 commits accumulated on the
> milestone branch — no separate merge step). 31 plan-row
> commits (`165916e..267607f`) land in 1:1 correspondence
> with `commits.md` round 9 ratification. `scope.md` round 22
> converged after 22 pi review rounds; `commits.md` round 9
> ratified after 9 pi review rounds. Per `plans/README.md`
> §"Patterns from prior milestones", retrospectives need
> adversarial review; m1 needed four rounds and m2 needed
> two — another pi pass is expected after this revision.
>
> Companion: `manual-validation.md` round 1 (Linux test /
> build / doc transcripts captured 2026-05-10; macOS CI URL,
> interactive smoke recording, and CI run URL all marked
> ⏳ pending the post-retrospective branch push).

This is the milestone-level review against `scope.md` round
22 and `commits.md` round 9 per `plans/README.md` Phase 3.
The five sections match m1 / m2's structure.

> **Numbering note.** `commits.md` enumerates plan rows
> c01–c31. The git log carries 31 implementation commits
> in 1:1 correspondence (no bundling, no docs-only
> insertions between plan rows during Phase 3). The 59
> docs-only commits in the m3 range are all Phase-2
> artifacts (scope rounds 2–22, commits rounds 1–9, pi
> review files) landed before c01.

---

## 1. Coverage

`scope.md` §I positive + negative matrices plus the
per-§ test enumerations (§S, §R, §F, §T, §B, §C2, §H6,
§M1) name on the order of 50 unique test files.
Behavioural coverage is complete; **four files** landed
under different names than scope, and the
`tui_subscribes_to_core_session_events.rs` row split
into three smaller landed files at the c25 reconciliation.
The reconciliation table below records each rename,
split, or indirect-coverage case. (Round 2 over-claimed
"name-for-name match" and underestimated the inventory
at "41 files"; round 3 corrects both.)

The `nix develop --impure --command cargo test
--manifest-path rafaello/Cargo.toml --workspace --features
test-fixture` acceptance gate (verbatim from `scope.md`
§"Acceptance summary"; the `test-fixture` feature
resolves to `rafaello-core/test-fixture` via the
`rafaello-core` crate's feature exposure) aggregates
**516 tests passed; 0 failed; 0 ignored** across 308
test binaries on Linux inside the devshell (`/tmp/m3-acceptance.log`,
captured 2026-05-10; transcript archived in
`manual-validation.md` §1).

### Scope-vs-landed file-name reconciliation

Four scope-named files do not match the landed file
names 1:1; the behaviours are covered, but the files
were renamed, split, or fold into adjacent setup. Recorded
explicitly so the on-disk inventory can be cross-checked
without surprise:

| `scope.md` test name | Landed file(s) | Reason |
|----------------------|----------------|--------|
| `renderer_pipeline_built_in_kinds.rs` | split into eight files: `renderer_builtin_text.rs`, `renderer_builtin_heading.rs`, `renderer_builtin_code_block.rs`, `renderer_builtin_thinking.rs`, `renderer_builtin_tool_call.rs`, `renderer_builtin_tool_result.rs`, `renderer_builtin_image.rs`, `renderer_builtin_error.rs` | c11/c12 — one test per built-in renderer per `commits.md` row split, per pi-9 polish: per-renderer files are easier to bisect than a single eight-case table. |
| `tui_subscribes_to_core_session_events.rs` | scope behaviour landed split across three files: `frontend_subscribes_to_core_session_events.rs` (c15, `dbfeebc` — broker-side fan-out to a registered frontend ACL), `tui_test_mode_logs_bus_events_to_stderr.rs` (c25, `8e87edf` — bus.event sentinel on TUI stderr; uses `core.lifecycle.demo` as the test topic, not `core.session.entry.finalized`), and `tui_test_mode_exits_on_test_done.rs` (c25 — `core.lifecycle.test_done` clean exit). Plus `rfl_chat_demo_bar.rs` (c31) and `rfl_chat_replay_withheld_until_frontend_ready.rs` (c30) exercise the actual `core.session.entry.finalized` topic end-to-end through the CLI/TUI path. The scope-named test's three behaviours (`core.session.entry.finalized` reception → stderr line → `test_done` exit) are individually covered, but no single landed file aggregates all three: the c25 split was pi-12 reconciliation to keep each test bisectable. |
| `frontend_register_with_broker.rs` | covered indirectly as setup in `frontend_subscribes_to_core_session_events.rs`, `frontend_publish_*.rs` (4 files), and `broker_register_frontend_*.rs` (2 files) | The positive happy-path "frontend lands in registry, guard drops cleanly" is the precondition for every frontend-layer integration test; no test-file specialised on registration alone landed because no row in `commits.md` had it as primary acceptance. **Recorded as a coverage gap of test-file granularity, not behaviour:** the path is exercised in setup of seven adjacent tests. Filed below. |
| `frontend_register_duplicate_rejected.rs` | landed as `broker_register_frontend_duplicate_rejected.rs` (c14, `79d4c0d`) | The behaviour is broker-side ACL enforcement, not a frontend-side surface; pi-14 reconciliation moved the file to the `broker_*` namespace to match m2's broker-test naming convention. |

### Positive matrix verification

The §I positive matrix is satisfied. Per-row mapping:

| `scope.md` positive test | Landed file | Commit |
|--------------------------|-------------|--------|
| `frontend_register_with_broker.rs` | indirect (see reconciliation row 3) | c14 setup paths |
| `session_store_round_trip.rs` | `session_store_round_trip.rs` | `4a76a2d` (c22) |
| `session_store_lock_fd_not_inherited_by_child.rs` | `session_store_lock_fd_not_inherited_by_child.rs` | `50156a5` (c21) |
| `session_controller_finalize_entry.rs` | `session_controller_finalize_entry.rs` | `82a1f6a` (c23) |
| `session_controller_replay_history.rs` | `session_controller_replay_history.rs` | `82a1f6a` (c23) |
| `renderer_pipeline_built_in_kinds.rs` | 8 split `renderer_builtin_*.rs` (see reconciliation row 1) | `b6e3791` (c11) + `0336ff2` (c12) |
| `renderer_pipeline_unknown_kind_falls_back_with_author_fallback.rs` | `renderer_pipeline_unknown_kind_falls_back_with_author_fallback.rs` | `6e62ad9` (c10) |
| `renderer_pipeline_unknown_kind_no_fallback_uses_default_callout.rs` | `renderer_pipeline_unknown_kind_no_fallback_uses_default_callout.rs` | `6e62ad9` (c10) |
| `renderer_pipeline_panic_falls_through_to_path_a.rs` | `renderer_pipeline_panic_falls_through_to_path_a.rs` | `6e62ad9` (c10) |
| `renderer_pipeline_renderer_err_falls_through_to_path_a.rs` | `renderer_pipeline_renderer_err_falls_through_to_path_a.rs` | `6e62ad9` (c10) |
| `renderer_capabilities_downgrade_unsupported_node.rs` | `renderer_capabilities_downgrade_unsupported_node.rs` | `6e62ad9` (c10) |
| `renderer_capabilities_downgrade_unsupported_raw_format.rs` | `renderer_capabilities_downgrade_unsupported_raw_format.rs` | `6e62ad9` (c10) |
| `supervisor_spawn_unwinds_after_register.rs` | `supervisor_spawn_unwinds_after_register.rs` | `adc1e8a` (c07) |
| `supervisor_spawn_post_register_reaps_child.rs` (Linux-only) | `supervisor_spawn_post_register_reaps_child.rs` | `adc1e8a` (c07) |
| `supervisor_spawn_unwinds_after_socketpair.rs` | `supervisor_spawn_unwinds_after_socketpair.rs` | `adc1e8a` (c07) |
| `supervisor_spawn_unwinds_post_spawn_pre_register.rs` | `supervisor_spawn_unwinds_post_spawn_pre_register.rs` | `adc1e8a` (c07) |
| `supervisor_spawn_unwinds_post_spawn_pre_register_fd_baseline.rs` | `supervisor_spawn_unwinds_post_spawn_pre_register_fd_baseline.rs` | `adc1e8a` (c07) |
| `frontend_handle_wait_resolves_on_child_exit.rs` | `frontend_handle_wait_resolves_on_child_exit.rs` | `7822575` (c20) |
| `frontend_handle_drop_does_not_leak_zombie.rs` (Linux-only) | `frontend_handle_drop_does_not_leak_zombie.rs` | `7822575` (c20) |
| `frontend_handle_wait_ready_resolves_on_signal.rs` | `frontend_handle_wait_ready_resolves_on_signal.rs` | `7822575` (c20) |
| `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs` | `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs` | `7822575` (c20) |
| `manifest_publishes_unknown_namespace_rejected.rs` | `manifest_publishes_unknown_namespace_rejected.rs` | `b8bff38` (c05) |
| `tui_subscribes_to_core_session_events.rs` | `frontend_subscribes_to_core_session_events.rs` (see reconciliation row 2) | c14 |
| `tui_handler_calls_frontend_ready.rs` | `tui_handler_calls_frontend_ready.rs` | `8e87edf` (c25) |
| `tui_test_mode_logs_bus_events_to_stderr.rs` | `tui_test_mode_logs_bus_events_to_stderr.rs` | `8e87edf` (c25) |
| `tui_test_mode_exits_on_test_done.rs` | `tui_test_mode_exits_on_test_done.rs` | `8e87edf` (c25) |
| `tui_test_mode_self_timeout_exits_zero.rs` | `tui_test_mode_self_timeout_exits_zero.rs` | `8e87edf` (c25) |
| `tui_sends_frontend_ready_after_handler_registration.rs` | `tui_sends_frontend_ready_after_handler_registration.rs` | `8e87edf` (c25) |
| `rfl_chat_demo_bar.rs` (headline) | `rfl_chat_demo_bar.rs` | `267607f` (c31) |
| `rfl_chat_resolves_tui_via_env_override.rs` | `rfl_chat_resolves_tui_via_env_override.rs` | `59e8b61` (c29) |
| `rfl_chat_relative_project_root_canonicalises.rs` | `rfl_chat_relative_project_root_canonicalises.rs` | `59e8b61` (c29) |
| `rfl_chat_replay_withheld_until_frontend_ready.rs` | `rfl_chat_replay_withheld_until_frontend_ready.rs` | `9d338c0` (c30) |

### Negative matrix verification

The §I negative matrix is satisfied. The 15 negative
rows in scope §I + §C2 (locked-session, project-root,
ready-timeout / before-ready / post-ready abnormal-exit)
all land:

| `scope.md` negative test | Landed file | Commit |
|--------------------------|-------------|--------|
| `frontend_publish_on_reserved_namespace_rejected.rs` | `frontend_publish_on_reserved_namespace_rejected.rs` | `dbfeebc` (c15) |
| `frontend_publish_two_segment_topic_rejected.rs` | `frontend_publish_two_segment_topic_rejected.rs` | `dbfeebc` (c15) |
| `frontend_publish_unknown_namespace_rejected.rs` | `frontend_publish_unknown_namespace_rejected.rs` | `dbfeebc` (c15) |
| `frontend_publish_outside_grant_rejected.rs` | `frontend_publish_outside_grant_rejected.rs` | `dbfeebc` (c15) |
| `frontend_register_unknown_attach_id_rejected.rs` (broker-layer) | `broker_register_frontend_unknown_attach_id_rejected.rs` (c14, `79d4c0d`) — broker-side `register_frontend()` ACL refusal; AND `frontend_register_unknown_attach_id_rejected.rs` (c18, `22a345c`) — `FrontendSupervisor::spawn` surfacing `AttachIdNotInAcl`. Both layers' refusals landed; the broker-layer file uses the m2 `broker_*` naming convention. | c14 + c18 |
| `frontend_register_duplicate_rejected.rs` | `broker_register_frontend_duplicate_rejected.rs` (see reconciliation row 4) | `79d4c0d` (c14) |
| `frontend_spawn_invalid_attach_id_rejected.rs` | `frontend_spawn_invalid_attach_id_rejected.rs` | `22a345c` (c18) |
| `frontend_spawn_relative_entry_path_refused.rs` | `frontend_spawn_relative_entry_path_refused.rs` | `22a345c` (c18) |
| `frontend_spawn_control_chars_in_path_refused.rs` | `frontend_spawn_control_chars_in_path_refused.rs` | `22a345c` (c18) |
| `frontend_spawn_entry_not_executable_refused.rs` | `frontend_spawn_entry_not_executable_refused.rs` | `22a345c` (c18) |
| `frontend_spawn_reserved_env_in_pass_refused.rs` | `frontend_spawn_reserved_env_in_pass_refused.rs` | `22a345c` (c18) |
| `frontend_spawn_reserved_env_in_set_refused.rs` | `frontend_spawn_reserved_env_in_set_refused.rs` | `22a345c` (c18) |
| `session_store_concurrent_open_errors.rs` | `session_store_concurrent_open_errors.rs` | `50156a5` (c21) |
| `session_store_locked_unknown_holder_errors.rs` | `session_store_locked_unknown_holder_errors.rs` | `50156a5` (c21) |
| `session_store_schema_mismatch_errors.rs` | `session_store_schema_mismatch_errors.rs` | `50156a5` (c21) |

### CLI negative tests (additional matrix)

`scope.md` §C2 enumerates `rfl chat`-shell-level error
paths. These are CLI-layer negatives, separate from the
broker / frontend / session core-layer negatives above:

| `scope.md` CLI negative test | Landed file | Commit |
|------------------------------|-------------|--------|
| `rfl_chat_locked_session_errors_with_holder_pid.rs` | same | `59e8b61` (c29) |
| `rfl_chat_locked_session_unknown_holder_errors.rs` | same | `59e8b61` (c29) |
| `rfl_chat_nonexistent_project_root_errors.rs` | same | `59e8b61` (c29) |
| `rfl_chat_frontend_exits_before_ready_errors.rs` | same | `59e8b61` (c29) |
| `rfl_chat_frontend_ready_timeout_errors.rs` | same | `59e8b61` (c29) |
| `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs` | same | `9d338c0` (c30) |

### Tests added beyond the matrix (examples)

The list below is illustrative, not exhaustive. Other
non-matrix tests required by individual `commits.md`
acceptance lines include compile-surface assertions and
unit-level seam tests.

- `m3_frontend_error_surface_compiles.rs` (per c14, c17,
  c18 acceptance rows) — compile-surface assertions
  matching the m1/m2 c02-style pattern.
- `tui_paint_panic_isolation` — `#[cfg(test)]` lib unit
  test in `rafaello-tui/src/paint.rs`, driven by
  `ratatui::backend::TestBackend`, exercising both
  `PaintAction::RunPanicking` and `PaintAction::RunReturningError`
  synthetic variants (c26 acceptance).
- `frontend_shutdown_dead_watch_paths.rs` (c19) — three
  dead-watch branches: `waitfailed_child_already_gone`,
  `reaperpanicked_child_already_gone`, and
  `live_terminates_after_sigterm`.
- `frontend_handle_shutdown_skips_kill_on_exited.rs` and
  `frontend_handle_shutdown_kills_on_waitfailed.rs` (c20)
  — the disarmable-Drop semantics matrix.
- `rfl_chat_harness_finalizes_nine_entries.rs` (c30) —
  harness-path positive that the `rfl_chat_demo_bar.rs`
  headline depends on for SQLite-row coverage.

All extras are additive; none replace a scope-named
behaviour.

### Coverage verdict

No behavioural coverage loss recorded. The three
m2-deleted unwind tests (m2 retro §5.1) returned in
c07 (`adc1e8a`) re-tied to the new `TestHooks`
3-inject-point seam, restoring the pre-register
socketpair-window coverage that m2 §5.1 left open.
No c-level test deletion occurred during Phase 3.

**Test-file granularity gap, recorded explicitly:**
`frontend_register_with_broker.rs` from scope §I did
not land as a stand-alone file; the positive
registration path is the precondition for seven
other frontend-layer integration tests and is
exercised in their setup blocks. A future driver
hardening pass may wish to land a dedicated
`frontend_register_*` file purely for grep-ability;
this is filed in §5.8 below as a non-blocking m4
follow-up.

The 516 tests / 0 failed result on Linux satisfies
`scope.md` §"Acceptance summary"'s **first**
bullet (Linux `cargo test`); the **second** bullet
(macOS CI green hard gate) remains pending the
post-retrospective branch push (`manual-validation.md`
§4); the **third** and **fourth** bullets (cargo
build green; cargo doc warning-free) are captured in
`manual-validation.md` §2 and §3.

---

## 2. Drift against overview / decisions / stream RFCs

The drift items below were **anticipated** by `scope.md`
§"Acceptance summary" — nine bullets pre-named the items
the m3 retrospective was expected to address. Following
the m1 / m2 pattern, this retrospective records the fix;
the actual `decisions.md` rows / overview patches /
Stream A + Stream E banners land as separate follow-up
commits on this branch before milestone close.

### 2.1 Stream E renderer-RFC drift patch (anticipated)

`streams/e-renderer/rfc-renderer-model.md` §7 (patch
ops), §8 (`frontend.hello`), and §9 (subprocess
`renderer.render`) describe v2-only mechanisms. m3
implements the v1 alternatives instead: panic-isolated
in-process rendering (no subprocess), `Capabilities`
constructed at `rfl chat` startup (no `frontend.hello`
handshake), and full-tree `RenderNode` republish per
finalize (no patch ops). Following the m1 banner
precedent, m3 lands a v1-status banner at the top of
`streams/e-renderer/rfc-renderer-model.md` pointing at
overview §10 / `decisions.md` rows 27–29. The body of
the RFC is not retroactively rewritten.

### 2.2 `PublisherIdentity::Frontend` schema additions to Stream A (anticipated)

Symmetric to m2's banner addition for `Plugin`. Stream
A's wire-schema banner expands from "m2 wire schemas"
to include `PublisherIdentity::Frontend { attach_id }`
once m3 promotes it via the bus changes in c14 / c15.
Follow-up commit on this branch updates the
`streams/a-security/rfc-security-model.md` banner.

### 2.3 Capabilities staging note in overview §10.1 (anticipated)

m3's TUI has compile-time-baked `Capabilities`
(`Capabilities::tui_default()` per c09), not a
`frontend.hello` handshake (per the row 27 deferral).
Overview §10.1's banner already says "v2 will
handshake"; this retrospective records the concrete
v1 indexing scheme: per-`attach_id` constants, with
`tui_default()` enumerating all 15 RenderNode variant
names + `raw_formats = {"ansi", "plain"}`. Follow-up
commit on this branch refreshes the §10.1 wording with
the concrete shape.

### 2.4 Replay over `core.session.entry.finalized` with `replay: bool` envelope flag (anticipated)

Round-2 collapse away from a separate `entry.replay`
topic. The canonical wire shape is the
`core.session.entry.finalized` event with envelope
fields `{ entry, tree, seq, replay: bool }`; `replay
= true` on history replay (c23
`SessionController::replay_history`), `replay = false`
on fresh entries (c23 `SessionController::finalize_entry`).
A new `decisions.md` row pins this. The
metadata-on-`EntryMetadata` alternative was rejected
during the round-2 → round-22 scope iterations on the
grounds that `replay` is a transport-level routing
fact, not entry content.

### 2.5 Broker error variant additions (anticipated)

Round 2 added `BrokerError::FrontendNotInAcl`,
`BrokerError::FrontendAlreadyRegistered`,
`BrokerError::FrontendNotRegistered`, and reshaped the
existing `PublishOutsideGrant` / `InvalidInReplyTo` to
take `publisher: Publisher` rather than `canonical:
CanonicalId`. The reshape is source-breaking for m2's
internal tests but contained to `rafaello-core`; c14
landed the variants and migrated the m2 broker tests
in the same commit. Follow-up commit on this branch
adds a `decisions.md` row recording the publisher-vs-
canonical reshape.

### 2.6 m1 publishes-grant unknown-namespace tightening (m2 retro §2.8)

Landed as the §M1 patch in c05 (`b8bff38`). The
regression baseline is
`manifest_publishes_unknown_namespace_rejected.rs`
under `rafaello-core/tests/`. m2 retro §2.8 had filed
this as a m3-or-m4 follow-up; m3 picked it up as the
self-contained §M1 commit because the m3 frontend
publish path (c14 / c15) shares the same namespace
authority machinery that the m1 manifest validator
guards.

### 2.7 m1 lock-side `check_lock_publish_topic` unknown-namespace gap (recorded for m4)

`check_publish_topic` (manifest side) was tightened
by §M1 / c05 to reject unknown top-level segments,
but `check_lock_publish_topic` (lock side) still
accepts them via its `_ => {}` arm. m3 leaves this
as runtime-only enforcement at the broker; the
rationale: hand-authored locks are an
`--allow-unsafe` user-override path where runtime
rejection is sufficient defence. Filed for m4 if a
user-facing failure surfaces.

### 2.8 `FrontendSupervisor` lock-correspondence claim extension (recorded)

Same v2 nice-to-have as m2 retro §2.6, now covering
both supervisors. The m3 `FrontendSupervisor` (c17 /
c18 / c20) does not assert lock-correspondence at
the API level any more strongly than `PluginSupervisor`
did at m2 close. Recorded for v2.

### 2.9 Fixture self-timeout (`RFL_FIXTURE_MAX_LIFETIME`) (anticipated, m2 retro §4.4 carryover)

Landed as the §L1 m2-fixture patch in c16 (`4db235d`).
Every long-running fixture mode (`respond_peer_call`,
`observer`, `signal_ready`, `hold_silent`,
`signal_ready_then_exit_n`, `frontend_bus_publish`)
reads `RFL_FIXTURE_MAX_LIFETIME` (seconds; default
60 if unset) and `std::process::exit(0)` on elapsed
time even without SIGTERM. The driver-side reaper
alternative (option 1 in m2 retro §4.4) was rejected
because it is operationally fragile (`pgrep -f` on
worktree paths) and only catches the driver's own
runs — local devs running `cargo test` outside the
driver lose the property. Option 2 is a 5-line code
change with zero operational surface.

### 2.10 m3 frontend ACL grants nothing on `publish_topics` (m4 / m5 handover, anticipated)

m3's `BrokerAcl` for the `tui` frontend has
`publish_topics = []` — the TUI never publishes on
the bus in m3. m4's first action will be to extend
the grant for `frontend.tui.user_message`; m5's for
`frontend.tui.confirm_answer`. This retrospective
files the ACL handover as anticipated, not as an m3
issue.

### 2.11 Verdict

Two of the ten drift items already landed as Phase-3
code commits (§2.6 in `b8bff38` / c05; §2.9 in
`4db235d` / c16). Two are recorded-only with no action
needed (§2.7 `check_lock_publish_topic` deferral;
§2.8 lock-correspondence v2 nice-to-have). One is an
m4 / m5 inflow item (§2.10 frontend ACL handover).
**The remaining five (§2.1 / §2.2 / §2.3 / §2.4 /
§2.5) are pending follow-up commits**, pre-named in
the "Follow-up commits on this branch" section below.
This retrospective records the canonical fix for each
pending item; the actual decisions.md / overview /
banner patches land as separate commits before
ratification, mirroring the m1 / m2 close pattern.

No additional unprompted drift surfaced during the
Phase 3 review.

---

## 3. Slipped or cut

### 3.1 No item from scope's lists slipped or was cut

All 31 plan rows in `commits.md` round 9 land 1:1.
No mid-Phase-3 restructure (m2 had a c22→c23
relocation in `d7c7705`) was needed.

### 3.2 No m3a / m3b split was triggered

m3's surface is large (frontend supervisor +
SessionStore + RenderPipeline + TUI + rfl chat) but
the per-commit walks held under the existing
`commits.md` plan without forcing a milestone split.
The pi-9 commits.md ratification's 31-commit
structure (12 groups) is what landed.

### 3.3 No extra-scope features added during implementation

Per-commit agents stayed within the row-prescribed
scope. No new public APIs, error variants, or test
files beyond the row enumerations were added by an
agent without explicit scope authorization.

---

## 4. Process notes for the next milestone driver

### 4.1 Per-commit-prompt inlining held up across the milestone

The pattern established in m2 — paste the full
`commits.md` row verbatim into the per-commit prompt,
plus the relevant `scope.md` excerpts inlined — held
up across all 31 m3 commits. Agents did not need to
read the milestone docs themselves; the prompt was
self-sufficient. Carried forward unchanged from m2
retro §4.7.

### 4.2 22 scope rounds + 9 commits rounds is m3's bracket

m3 took **22 scope review rounds** (m1: 4, m2: 11) and
**9 commits review rounds** (m1: ~3, m2: 5). The
larger bracket reflects m3's broader surface (TUI +
sessions + rendering + frontend supervisor) and the
two collapse events during scope iteration: round-2
collapse of separate `entry.replay` topic into the
`replay: bool` flag, and round-5 collapse of c08 +
c09 (Entry + RenderNode) when implementation
coupling made the split unimplementable. Future
milestones with comparable surface should budget 15+
scope rounds and 8+ commits rounds.

### 4.3 Round-3 `commits.md` renumber regex hazard

The round-3 commits.md restructure (move fixture
extensions ahead of frontend group, consolidate
Phase B + FrontendHandle into c20) involved a
multi-step renumber. The naive `sed
's/c\([0-9]\+\)/c\$((NEW))/g'` approach
double-renumbered when an earlier `cN` appeared in
a later row's `Depends on.` field. Fix: reset via
`git checkout HEAD -- commits.md` and apply a
single-pass mapping `{c22→c20, c24→c22, ...}`.
Documented for the next driver: large
commits.md restructures need single-pass mapping,
not iterative `sed`.

### 4.4 Pi WebSocket disconnect mid-write (recurring)

Twice during scope.md rounds 5 and 6, pi disconnected
mid-write. The driver nudged with "resume the round-N
review" and pi resumed from prior context. No data
loss observed. Future drivers should treat
disconnects as transient and resume rather than
restart the round.

### 4.5 Pi orphan-review pattern (three occurrences)

Three times pi printed reviews to chat without saving
the file (`pi-review-{5,6,8}.md`); the driver
committed those manually. The recurring nudge — "save
to `<path>` and `git add` + `git commit
--no-gpg-sign`" — does not always fire. Carried
forward from m2; consider a stronger structured
prompt template for m4.

### 4.6 c08+c09 round-5 collapse caught by pi

Pi-5 M3 spotted that the planned c08 (Entry) + c09
(RenderNode) split was unimplementable: the
`tool_result.content: RenderNode` field couples the
two types at the field level. Driver collapsed them
into a single c08 commit during round 5. Future
drivers: treat field-level type coupling as a hard
no for split commits.

### 4.7 `Cargo.lock` stash-before-ff-merge held (m2 retro §4.5 carryover)

The m2 retro §4.5 mitigation — `git stash push
rafaello/Cargo.lock` before each ff-merge, then `git
stash drop` — held across all 31 m3 ff-merges. No
ff-merge was silently aborted by a dirty Cargo.lock.

### 4.8 Per-commit cleanup contract held (m2 retro §4.6 carryover)

Worktree creation in `/home/luiz/lab-wt/m3-cNN/`,
prek symlink, `direnv allow .`, tmux claude session
spawn, `tmux-wait` on `"esc to interrupt"`, ff-merge
with Cargo.lock stash, worktree removal — pattern
held without exception across all 31 commits. The
`pre-commit-config.yaml` symlink fix from m2 §4.6
carries forward unchanged.

### 4.9 `c01` first-ff-merge failure recovery

After the orphan-review commit landed on
rafaello-v0.1, `c01`'s base was no longer the tip.
ff-merge failed cleanly; recovery was a single
`git cherry-pick agents/m3/c01`. Documented for the
next driver: when a docs-only commit lands during
Phase 3 between two per-commit agent dispatches,
the next ff-merge needs a cherry-pick fallback.

### 4.10 Six "zero-blocker" pi rounds during scope iteration

Of the 22 scope rounds, six rounds returned zero
blockers (rounds 14, 16, 17, 19, 21, 22). Rather
than declaring convergence at round 14, the driver
continued through round 22 because each subsequent
round still surfaced high-priority polish items.
The implicit convergence rule — "ratify when pi
returns zero blockers AND zero high-priority items
AND zero medium-priority items two rounds in a row"
— matches m2's pattern but was not previously
documented; future drivers should adopt this as the
explicit convergence test.

---

## 5. Known issues to track

### 5.1 macOS CI green is a hard ratification gate (`scope.md` §"Acceptance summary" round-6 tightening)

m3 ships a user-facing TUI/session surface with
cross-platform consequences (flock semantics,
terminal handling, fd inheritance). `scope.md`
round-6 promoted macOS CI from optional to a hard
ratification gate. **Status: pending the
post-retrospective push to GitHub.** The driver
captures the macOS run URL in `manual-validation.md`
once the branch pushes and the workflow completes.
Tests gated `#[cfg(target_os = "linux")]` (e.g.
`frontend_handle_drop_does_not_leak_zombie.rs`,
`supervisor_spawn_post_register_reaps_child.rs`)
are exempted; tests that fail on macOS for other
reasons must be fixed in m3, not deferred.

### 5.2 No flakes observed in the recorded run

The 516-test run captured 2026-05-10 inside the
devshell on Linux returned 0 failures across 308
test binaries. No retries needed. The
fixture-self-timeout patch (§2.9) appears to have
removed the m2-era flake source where panicked
tests left fixture processes alive; no residual
flake tax was billed during the m3 walk.

### 5.3 No clippy warnings suppressed by agents

All per-commit walks closed clippy clean; no
`#[allow(...)]` was introduced by an agent during
Phase 3. The pre-commit clippy hook (`cargo clippy
-- -D warnings`) gated every commit.

### 5.4 No `cargo doc` warnings (regression check vs m2 §5.2)

m2 retro §5.2 closed a `private_intra_doc_links`
warning. m3's `cargo doc --workspace --no-deps`
returns clean. The m2 fix held under m3's surface
expansion.

### 5.5 No dead-watch race observed in `shutdown_with_outcome`

The c19 `shutdown_with_outcome` seam — split
live-watch / dead-watch with `signal_fn` / `probe_fn`
mockable closures — was tested via three branches
and did not surface a race in any post-c19 commit's
test run. The `kill(pid, 0)` liveness probe pattern
(via `nix::sys::signal::kill(pid, None)`) is
recorded as the canonical no-op probe.

### 5.6 Disarmable FrontendHandle Drop semantics

The c20 `FrontendHandle` Drop pattern uses
`Option<>`-wrapped fields + `take()`-then-shutdown.
The three Drop branches (Exited-skip /
abnormal-best-kill / None-best-kill) all hit during
`frontend_handle_drop_*` tests. No Drop-side
unwind escape observed.

### 5.7 Cleanup-guard contract in `rfl chat`

The c30 cleanup-guard contract — single
`Option<handle>::take()`, single `forwarder.await`
drain, regardless of which inner step errored —
held across all `rfl_chat_*` integration tests.
The pattern is documented in `scope.md` §C2 step 8
verbatim and the c30 implementation matches it
line-for-line.

### 5.9 rafaello-tui-test-harness macOS port (BLOCKING ratification — code follow-up)

`rafaello/crates/rafaello-tui/tests/common/mod.rs` is
gated `#![cfg(target_os = "linux")]` because it calls
`nix::sys::socket::socketpair` with the
`SockFlag::SOCK_CLOEXEC` flag, which is unavailable on
macOS (the same failure mode m2 retro §5.7 captured for
`rfl-bus-fixture` and fixed in `7db9da8` via an
`fcntl(F_SETFD, FD_CLOEXEC)` fallback). All five
`rafaello-tui/tests/*.rs` integration tests inherit the
gate transitively:

- `tui_handler_calls_frontend_ready.rs`
- `tui_test_mode_logs_bus_events_to_stderr.rs`
- `tui_test_mode_exits_on_test_done.rs`
- `tui_test_mode_self_timeout_exits_zero.rs`
- `tui_sends_frontend_ready_after_handler_registration.rs`

These exercise central m3 TUI/session behaviours and
are NOT platform-inherent Linux assertions (unlike
`frontend_handle_drop_does_not_leak_zombie.rs` which
reads `/proc/<pid>` zombie state, or
`supervisor_spawn_unwinds_*_fd_baseline.rs` which read
`/proc/self/fd`). Pi-review-2 B3 caught this:
`scope.md` round-6 made macOS CI a **hard ratification
gate** precisely so cross-platform TUI/session
behaviour is proven; a green macOS run that skips all
five files would not satisfy that gate.

**Required follow-up code commit before m3
ratification:** apply the m2 `7db9da8` SOCK_CLOEXEC →
`fcntl` fallback pattern to
`rafaello-tui/tests/common/mod.rs`, then drop the
blanket `#![cfg(target_os = "linux")]` from each of
the five test files. The m2 retro records this as a
proven 5-line patch.

### 5.8 `frontend_register_with_broker.rs` test-file granularity gap (m4 follow-up)

`scope.md` §I positive matrix names a stand-alone
positive test for the frontend-registration happy
path. No file by that name landed; the path is
exercised only as setup in seven adjacent integration
tests (`frontend_subscribes_to_core_session_events.rs`,
the four `frontend_publish_*.rs` files, and the two
`broker_register_frontend_*.rs` files). The
behavioural coverage is complete; the file-name
granularity is the gap. Recorded as a non-blocking m4
hardening pass — landing a dedicated
`frontend_register_with_broker.rs` would make the
positive path grep-discoverable without changing
behaviour.

---

## Follow-up commits on this branch

Per the m1 / m2 precedent, the drift items in §2 land
as **separate commits on this branch before milestone
close**. The list below pre-names them; items 1–5 are
docs-only/banner-only patches addressing §2 drift; item
6 is a code patch addressing §5.9.

1. `docs(rafaello-streams-e): v1-status banner — point at
   decisions rows 27–29, no body rewrite` — addresses §2.1.
2. `docs(rafaello-streams-a): banner expand — include
   PublisherIdentity::Frontend { attach_id }` — addresses §2.2.
3. `docs(rafaello-overview): §10.1 — concrete v1 Capabilities
   indexing scheme + 15 RenderNode variant names + raw_formats
   set` — addresses §2.3.
4. `docs(rafaello-decisions): replay envelope row — replay:
   bool flag on core.session.entry.finalized` — addresses §2.4.
5. `docs(rafaello-decisions): BrokerError variant additions +
   PublishOutsideGrant / InvalidInReplyTo reshape to take
   publisher: Publisher` — addresses §2.5.
6. `fix(rafaello-tui::tests/common): SOCK_CLOEXEC fallback via
   fcntl — un-gate rafaello-tui integration tests on macOS`
   — addresses §5.9. Mirrors m2 `7db9da8`.

Items §2.6 (m1 publishes-grant tightening) and §2.9
(fixture self-timeout) landed as Phase-3 code commits
(c05 and c16 respectively) and need no follow-up.

Items §2.7 (m1 lock-side gap) and §2.8 (lock-correspondence
extension) are recorded-only — no follow-up commit needed.

Item §2.10 (m3 frontend ACL handover) is an m4 / m5
inflow item, not an m3 follow-up.

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
is the build gate's exact form (the explicit `rafaello-core/`
prefix is required there because `--bins` does not
auto-enable the feature).

| `scope.md` §"Acceptance summary" bullet | Status |
|-----------------------------------------|--------|
| Every named test in §"Positive" / §"Negative" matrices implemented and passing | ✅ §1 (with 4 file-name reconciliations recorded) |
| `cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture` green on Linux inside devshell | ✅ 516/0/0 captured 2026-05-10 (`manual-validation.md` §1) |
| **macOS CI green** (hard gate) | ⏳ pending the post-retrospective branch push (`manual-validation.md` §4 + §5.1 below) |
| `cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture` green | ✅ captured 2026-05-10 (`manual-validation.md` §2) |
| `cargo doc --manifest-path rafaello/Cargo.toml --workspace --no-deps` warning-free | ✅ captured 2026-05-10 (`manual-validation.md` §3) |
| `manual-validation.md` records manual-validation list | ⏳ partial — §1 / §2 / §3 ✅ archived round 1; §4 (macOS CI URL) / §5 (interactive smoke) / §6 (CI run URL) ⏳ pending the post-retrospective push |
| `retrospective.md` written with anticipated drift addressed | ⏳ partial — document round 3 records the canonical fixes, but the five required follow-up commits (§2.1 / §2.2 / §2.3 / §2.4 / §2.5) and the §5.9 macOS-harness code commit have NOT YET LANDED |
| Stream E renderer-RFC drift patch | ⏳ §2.1 — follow-up commit pre-named, NOT YET LANDED |
| `PublisherIdentity::Frontend` Stream A schema additions | ⏳ §2.2 — follow-up commit pre-named, NOT YET LANDED |
| Capabilities staging note in overview §10.1 | ⏳ §2.3 — follow-up commit pre-named, NOT YET LANDED |
| Replay-via-`entry.finalized` decisions row | ⏳ §2.4 — follow-up commit pre-named, NOT YET LANDED |
| Broker error variant additions decisions row | ⏳ §2.5 — follow-up commit pre-named, NOT YET LANDED |
| m1 publishes-grant unknown-namespace tightening | ✅ landed in c05 (`b8bff38`) |
| m1 lock-side `check_lock_publish_topic` deferral | §2.7 — recorded for m4, no commit needed |
| `FrontendSupervisor` lock-correspondence claim extension | §2.8 — recorded for v2, no commit needed |
| Fixture self-timeout documentation | ✅ landed in c16 (`4db235d`) |
| m3 frontend ACL `publish_topics = []` handover to m4 | §2.10 — anticipated, no commit needed |

---

m3 ratifies once macOS CI captures green and the five
pre-named follow-up commits land. The ratification
sequence mirrors m2: pi reviews this retrospective →
driver lands drift fixes + macOS CI URL in
`manual-validation.md` → pi re-reviews → driver merges
or declares the milestone closed (since m3 commits
accumulated on `rafaello-v0.1` directly, no separate
merge step is needed).
