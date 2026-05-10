# m3 — sessions, local-spawned TUI, built-in rendering — retrospective

> **Status:** draft round 1 on 2026-05-10 by the milestone
> driver. Worktree `/home/luiz/lab` directly on
> `rafaello-v0.1` (m3 commits accumulated on the milestone
> branch — no separate merge step). 31 plan-row commits
> (`165916e..267607f`) land in 1:1 correspondence with
> `commits.md` round 9 ratification. `scope.md` round 22
> converged after 22 pi review rounds; `commits.md` round 9
> ratified after 9 pi review rounds. Per `plans/README.md`
> §"Patterns from prior milestones", retrospectives need
> adversarial review; m1 needed four rounds and m2 needed
> two — another pi pass is expected after this draft.
>
> Companion: `manual-validation.md` (driver-owned, written
> alongside this retrospective).

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

Every named test in `scope.md` §I integration matrix and the
per-§ test enumerations is implemented under
`rafaello/crates/rafaello-core/tests/`,
`rafaello/crates/rafaello-tui/tests/`, and
`rafaello/crates/rafaello/tests/`. The
`cargo test --workspace --features rafaello-core/test-fixture`
acceptance gate (`scope.md` §"Acceptance summary") aggregates
**516 tests passed; 0 failed; 0 ignored** across 308 test
binaries on Linux inside the devshell (run captured
2026-05-10).

### Scope-vs-landed file-name reconciliation

No 1:1 deviations from the scope-named test files were
detected during Phase 3. The full per-test reconciliation
table is omitted because the scope and the on-disk file
listing match name-for-name.

### Positive matrix verification

The §I positive matrix is satisfied. Spot checks:

| scope.md test file | Landed in | Notes |
|--------------------|-----------|-------|
| `rfl_chat_demo_bar.rs` | `267607f` (c31) | Headline: nine entries → nine `bus.event` lines + nine SQLite rows. |
| `rfl_chat_harness_finalizes_nine_entries.rs` | `9d338c0` (c30) | Harness path positive. |
| `rfl_chat_replay_withheld_until_frontend_ready.rs` | `9d338c0` (c30) | Combined-stream line-order proof. |
| `rfl_chat_resolves_tui_via_env_override.rs` | `59e8b61` (c29) | End-to-end env override + sentinel. |
| `rfl_chat_relative_project_root_canonicalises.rs` | `59e8b61` (c29) | §C2 step 1 canonicalize. |
| `tui_handler_calls_frontend_ready.rs` | `8e87edf` (c25) | Headless TUI → parent ready RPC. |
| `tui_test_mode_logs_bus_events_to_stderr.rs`, `tui_test_mode_exits_on_test_done.rs`, `tui_test_mode_self_timeout_exits_zero.rs`, `tui_sends_frontend_ready_after_handler_registration.rs` | `8e87edf` (c25) | Headless-mode contract. |
| `frontend_handle_*` (4 tests, signal/exit-before/timeout/post-ready) | `7822575` (c20) | Phase-B + handle lifecycle. |
| `frontend_shutdown_dead_watch_paths.rs` | `2f4cea9` (c19) | `shutdown_with_outcome` seam. |
| `session_store_round_trip.rs`, `session_store_seq_monotonic.rs` | `4a76a2d` (c22) | SessionStore round-trip + seq. |
| `session_store_lock_*` (3 tests) | `50156a5` (c21) | Flock + lock-first ordering + fd-not-inherited. |
| `session_controller_finalize_entry.rs`, `session_controller_replay_history.rs` | `82a1f6a` (c23) | Controller publish path. |
| `renderer_pipeline_path_*` (A/B/C — 3 tests) | `6e62ad9` (c10) | Three-path pipeline. |
| `entry_*_round_trip.rs` (9 tests) | `cff03b6` (c08) | Entry + RenderNode constructors. |
| `manifest_publishes_unknown_namespace_rejected.rs` | `b8bff38` (c05) | m1 §M1 publishes-grant tightening. |
| `supervisor_*_inject_fault_*` (3 tests) | `adc1e8a` (c07) + `779724d` (c06) | TestHooks 3 inject points (m2 §5.1 carryover). |
| `tui_paint_panic_isolation` lib unit test | `5f9ac1b` (c26) | `Painter::draw_with_panic_isolation` seam. |
| `workspace_bin_path_resolves_*` (2 tests) | `b87a74f` (c28) | Helper smoke tests. |

### Negative matrix verification

The §I negative matrix is satisfied:

| scope.md test file | Landed in |
|--------------------|-----------|
| `rfl_chat_locked_session_errors_with_holder_pid.rs` | `59e8b61` (c29) |
| `rfl_chat_locked_session_unknown_holder_errors.rs` | `59e8b61` (c29) |
| `rfl_chat_nonexistent_project_root_errors.rs` | `59e8b61` (c29) |
| `rfl_chat_frontend_exits_before_ready_errors.rs` | `59e8b61` (c29) |
| `rfl_chat_frontend_ready_timeout_errors.rs` | `59e8b61` (c29) |
| `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs` | `9d338c0` (c30) |
| `bus_publish_unknown_publisher_kind_rejected.rs` (frontend) | `dbfeebc` (c15) |
| `bus_register_frontend_not_in_acl_rejected.rs` | `79d4c0d` (c14) |
| `bus_register_frontend_duplicate_rejected.rs` | `79d4c0d` (c14) |
| `frontend_handle_drop_*` (Linux-only zombie test) | `7822575` (c20) |

### Tests added beyond the matrix (examples)

- `m3_types_compile.rs` patterns (per c14, c17, c18) —
  build-only compile-surface assertions matching the
  m1/m2 c02-style pattern.
- `tui_paint_panic_isolation` lib unit test — driven by
  `TestBackend` (ratatui), exercises both
  `RunPanicking` and `RunReturningError` synthetic
  PaintAction variants.
- `frontend_shutdown_dead_watch_paths.rs` — three
  dead-watch branches (`waitfailed_child_already_gone`,
  `reaperpanicked_child_already_gone`,
  `live_terminates_after_sigterm`).

### Coverage verdict

No coverage loss recorded. The three m2-deleted unwind
tests (m2 retro §5.1) returned in c07 (`adc1e8a`) re-tied
to the new `TestHooks` 3-inject-point seam (pre-spawn,
post-spawn-pre-register, post-register), restoring the
pre-register socketpair-window coverage that m2 §5.1 left
open. No c-level test deletion occurred during Phase 3.

The 516 tests / 0 failed result on Linux satisfies
`scope.md` §"Acceptance summary"'s first three bullets
(workspace tests; build of all three bins via the explicit
`--features rafaello-core/test-fixture` flag — verified
during c25 macOS CI gating; cargo doc warning-free).

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

All nine drift items pre-named in `scope.md`
§"Acceptance summary" are addressed by either the
follow-up commits enumerated below or by code already
landed during Phase 3 (§2.6 / §2.9). No additional
unprompted drift surfaced during the Phase 3 review.

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

---

## Follow-up commits on this branch

Per the m1 / m2 precedent, the drift items in §2 land
as **separate commits on this branch before milestone
close**. The list below pre-names them; each is a
docs-only or banner-only patch.

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

Items §2.6 (m1 publishes-grant tightening) and §2.9
(fixture self-timeout) landed as Phase-3 code commits
(c05 and c16 respectively) and need no follow-up.

Items §2.7 (m1 lock-side gap) and §2.8 (lock-correspondence
extension) are recorded-only — no follow-up commit needed.

Item §2.10 (m3 frontend ACL handover) is an m4 / m5
inflow item, not an m3 follow-up.

---

## Acceptance summary check

| `scope.md` §"Acceptance summary" bullet | Status |
|-----------------------------------------|--------|
| Every named test in §"Positive" / §"Negative" matrices implemented and passing | ✅ §1 |
| `cargo test --workspace --features test-fixture` green on Linux inside devshell | ✅ 516/0/0 captured 2026-05-10 |
| **macOS CI green** (hard gate) | ⏳ pending push (§5.1) |
| `cargo build --workspace --bins --features rafaello-core/test-fixture` green | ✅ verified during c25 / c27 |
| `cargo doc --workspace --no-deps` warning-free | ✅ verified post-c31 |
| `manual-validation.md` records manual-validation list | ⏳ writing alongside this retro |
| `retrospective.md` written with anticipated drift addressed | ⏳ this document |
| Stream E renderer-RFC drift patch | §2.1 — follow-up commit pre-named |
| `PublisherIdentity::Frontend` Stream A schema additions | §2.2 — follow-up commit pre-named |
| Capabilities staging note in overview §10.1 | §2.3 — follow-up commit pre-named |
| Replay-via-`entry.finalized` decisions row | §2.4 — follow-up commit pre-named |
| Broker error variant additions decisions row | §2.5 — follow-up commit pre-named |
| m1 publishes-grant unknown-namespace tightening | ✅ landed in c05 (`b8bff38`) |
| m1 lock-side `check_lock_publish_topic` deferral | §2.7 — recorded for m4 |
| `FrontendSupervisor` lock-correspondence claim extension | §2.8 — recorded for v2 |
| Fixture self-timeout documentation | ✅ landed in c16 (`4db235d`) |
| m3 frontend ACL `publish_topics = []` handover to m4 | §2.10 — anticipated, no action |

---

m3 ratifies once macOS CI captures green and the five
pre-named follow-up commits land. The ratification
sequence mirrors m2: pi reviews this retrospective →
driver lands drift fixes + macOS CI URL in
`manual-validation.md` → pi re-reviews → driver merges
or declares the milestone closed (since m3 commits
accumulated on `rafaello-v0.1` directly, no separate
merge step is needed).
