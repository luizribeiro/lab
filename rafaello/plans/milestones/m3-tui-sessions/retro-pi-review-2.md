# m3 retrospective.md — pi review round 2

Review target: `rafaello/plans/milestones/m3-tui-sessions/retrospective.md` at `75bde52`.

Verdict: **not ratifiable yet**. Round 2 fixes several round-1 issues, but the draft still contains factual coverage mismatches and still marks acceptance/drift items green while required evidence and follow-up commits are explicitly pending.

## Blocking findings

### B1. Coverage inventory count and several mappings are still wrong

`retrospective.md` now says scope §I plus per-§ enumerations name “a total of 41 test files.” A direct extraction of backticked `*.rs` names from `scope.md` between `#### Positive matrix` and `### H — test harness` yields **49** unique matrix filenames, before counting additional per-§ tests such as `tui_handler_calls_frontend_ready.rs` / `tui_test_mode_*`.

There are also concrete row errors:

- `tui_subscribes_to_core_session_events.rs` is mapped to `frontend_subscribes_to_core_session_events.rs` and c14. The landed file was introduced in c15 (`dbfeebc`), and it is a broker/frontend fan-out test, not the scope-described TUI-process test that spawns `rfl-tui`, publishes `core.session.entry.finalized`, sees stderr, and exits on `core.lifecycle.test_done`.
- `frontend_register_unknown_attach_id_rejected.rs` is marked as c14 (`79d4c0d`), but the landed file by that exact name is c18 (`22a345c`) and tests `FrontendSupervisor::spawn` surfacing `AttachIdNotInAcl`. The direct broker-layer c14 test is `broker_register_frontend_unknown_attach_id_rejected.rs`.
- `frontend_register_with_broker.rs` is recorded as only indirect setup coverage, then §1 still says every named behaviour is implemented. That can be acceptable only if the indirect tests are enumerated precisely and the missing standalone file is not simultaneously counted as a landed matrix file.

Fix: rerun the matrix inventory from `scope.md`, correct the count, and make each rename/indirect row name the actual landed file, commit, and behaviour boundary.

### B2. The `tui_subscribes_to_core_session_events.rs` behaviour is not reconciled accurately

The scope row for `tui_subscribes_to_core_session_events.rs` is specific: spawn `rfl-tui` in `RFL_TUI_TEST_MODE`, publish one `core.session.entry.finalized`, assert the TUI logs it on stderr, then exit on `core.lifecycle.test_done`.

Round 2 says this was actually “broker-layer enforcement” covered by `frontend_subscribes_to_core_session_events.rs`. That is not the same behaviour. The closest landed coverage appears split across:

- `tui_test_mode_logs_bus_events_to_stderr.rs` — logs a parent-sent `bus.event`, but uses `core.lifecycle.demo`, not `core.session.entry.finalized`, and does not prove test-done exit.
- `tui_test_mode_exits_on_test_done.rs` — proves `core.lifecycle.test_done` exit separately.
- `rfl_chat_demo_bar.rs` / replay tests — see `core.session.entry.finalized` lines through the CLI/TUI path.

Fix the reconciliation to state this split honestly, or add the exact scope-named TUI integration test.

### B3. macOS hard-gate wording misses that key m3 TUI tests are Linux-only

The retrospective/manual-validation repeatedly say macOS CI is pending and that only explicitly Linux-gated tests are exempt. But all `rafaello-tui/tests/*` integration tests are currently `#![cfg(target_os = "linux")]`, including the headless bus-event logging, `frontend.ready`, self-timeout, and handler-registration ordering tests. These are central m3 user-facing TUI/session behaviours, not obviously inherent Linux-only process/fd assertions like the zombie or `/proc` fd-baseline tests.

If macOS CI skips these tests, then a green macOS job will not provide the cross-platform TUI coverage that `scope.md` round-6 made a hard gate. The retrospective should either justify each Linux-only exemption as platform-inherent, or treat this as an m3 coverage/CI blocker to fix before ratification.

### B4. Acceptance checklist still uses green status for pending acceptance items

The round-2 text is clearer in §2.11, but the final table still over-greens:

- `manual-validation.md records manual-validation list` is marked ✅ even though manual-validation §4, §5, and §6 are all pending.
- `retrospective.md written with anticipated drift addressed` is marked ✅ even though the five required drift patches immediately below are “NOT YET LANDED.”

This repeats the round-1 ambiguity: a reader can still infer that the acceptance bullet is done while its required sub-items are pending. Mark these rows as ⏳ / partial until the macOS CI URL, interactive smoke, CI run URL, and five drift commits land.

### B5. Cargo evidence is summarized but not durably archived as transcripts

`retrospective.md` says the Linux cargo transcript is archived in `manual-validation.md` §1, but `manual-validation.md` records only aggregate counts and points to transient `/tmp/m3-acceptance.log`, `/tmp/m3-build.log`, and `/tmp/m3-doc.log`. That is better than round 1, but it is not an archived transcript in the milestone docs.

Fix either the wording (“aggregate recorded; raw logs were transient”) or paste enough tail/summary output into `manual-validation.md` that future reviewers do not depend on `/tmp` files.

## Non-blocking notes / polish

- `m3_frontend_error_surface_compiles.rs` is now the actual compile-surface file; remove “or similar landed name.”
- `manual-validation.md` §4 says macOS should pass “the same 516 less Linux exemptions.” Because many feature-gated/Linux-only tests are skipped, give an expected macOS count or avoid promising a specific arithmetic.
- The review filename in the status banner says `retro-pi-review-1.md`; that matches the new file name, but previous milestones used `retrospective-pi-review*.md`. No issue, just keep naming consistent from here.
