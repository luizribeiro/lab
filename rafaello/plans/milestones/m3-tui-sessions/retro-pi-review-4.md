# m3 retrospective.md — pi review round 4

Review target: `rafaello/plans/milestones/m3-tui-sessions/retrospective.md` at `82e6189`.

Verdict: **retrospective round 4 is acceptable as a pre-ratification status document, but m3 is not ratifiable yet**. The round-3 blockers I raised are addressed in substance: the `tui_subscribes_to_core_session_events.rs` split is now honestly represented, the macOS TUI-harness root cause no longer blames non-existent `SOCK_CLOEXEC` usage, and the final ratification checklist includes the sixth code follow-up.

## Blocking findings

None against the retrospective text at this revision.

The milestone remains blocked by the items the retrospective itself now lists:

1. The five docs/drift follow-up commits (§2.1–§2.5).
2. The §5.8 `rafaello-tui` integration-test macOS un-gating code commit.
3. A green macOS CI run that actually executes the newly un-gated TUI tests.
4. The real interactive `rfl chat` smoke recording.
5. Final Linux+macOS workflow URL capture in `manual-validation.md`.

Those are expected next-state blockers, not additional retrospective-draft defects.

## Non-blocking notes / polish before final archive

- `retrospective.md` says the `tui_subscribes_to_core_session_events.rs` behaviour is “split across four files,” but the row names five landed files if both CLI end-to-end tests are counted (`frontend_subscribes...`, two `tui_test_mode...`, `rfl_chat_demo_bar.rs`, and `rfl_chat_replay_withheld_until_frontend_ready.rs`). Cosmetic; the substance is clear.
- The acceptance table row for `retrospective.md written with anticipated drift addressed` still says “document round 3 records...” even though this is round 4.
- §2 intro says “nine bullets” while §2.11 correctly counts ten drift items. Prefer “anticipated drift items” or “ten items” to avoid another count nit.
- The coverage verdict says the Linux test run satisfies the acceptance summary’s “first bullet (Linux cargo test)”; in `scope.md`, the first bullet is named-test coverage and the Linux cargo-test command is the second bullet. The final acceptance table is correct, so this is just prose cleanup.
