# m6 commits.md Phase K amendment pi review (round 9)

> Verdict: BLOCKING.
> Counts: B/2 M/1 N/1

Reviewed Phase K only (`cK1`..`cK6`) from the `6a5779c`
`commits.md` amendment, against owner Route 1 (`891b93a`), the live
recapture finding, and current `rafaello-tui` / gate code.

## Blocking

### B1 — confirm-overlay live-shape citations point at the wrong modules

Phase K repeatedly says the queue / painter / ticker live in
`lib.rs` or `paint.rs`, but the current symbols live in
`rafaello/crates/rafaello-tui/src/confirm.rs`:

- `ConfirmQueue`: `confirm.rs:33` (not `lib.rs:33`).
- `ConfirmQueue::enqueue`: `confirm.rs:42`.
- `ConfirmQueue::head_overlay`: `confirm.rs:78`.
- `ConfirmQueue::handle_confirm_resolved`: `confirm.rs:102`.
- `ConfirmQueue::handle_confirm_reply`: `confirm.rs:109`.
- `run_ttl_ticker`: `confirm.rs:127` (not `lib.rs:127`).
- `paint_confirm_overlay`: `confirm.rs:160` (not `lib.rs:160` /
  `paint.rs`).
- `CONFIRM_RESOLVED_TOPIC`: `confirm.rs:22` (not `lib.rs:22`).

`lib.rs` does have `InputMode` (`lib.rs:30`),
`overlay_from_confirm_request` (`lib.rs:98`), `handle_overlay_key`
(`lib.rs:147`), and `CONFIRM_REQUEST_TOPIC` /
`CONFIRM_ANSWER_TOPIC` (`lib.rs:15`, `lib.rs:17`). `paint.rs` is the
plain `RenderNode` painter and has no confirm-overlay painter.

Fix the Phase K intro plus cK3/cK6 row bodies to cite/import the live
`rafaello_tui::confirm::*` symbols before per-commit prompts are built.

### B2 — cK4 requires a `t`/Toggle path that does not exist in the live m5a shape

cK4 says production overlay handling wires `a`/`d`/`s`/`t`, includes a
sibling test for `t` (Toggle), and cites this as matching
`Answer::from_key` at `lib.rs:68`. Live `Answer::from_key` supports:

- allow: `y` / `a` / `Enter` (`lib.rs:70`),
- deny: `n` / `d` / `Esc` (`lib.rs:71`),
- always-allow-session: `s` (`lib.rs:72`).

There is no `Toggle` variant and no `t` key. m5a c25 says the same
`y/a/Enter`, `n/d/Esc`, `s` shape. Since Phase K says it is wiring
existing primitives, not adding library semantics, cK4 should drop
`t`/Toggle and align acceptance with the existing helper, or else get
explicit owner/scope authorization for a new primitive.

## Major

### M1 — cK3/cK4 omit the own-answer queue-pruning path

cK3 mentions pruning on `core.session.confirm_resolved`, but not on
`core.session.confirm_reply`. The live queue has both
`handle_confirm_resolved` and `handle_confirm_reply`, and m5a c26's
shape explicitly drops entries resolved by the TUI's own answer on the
matching `core.session.confirm_reply` event.

If cK4 only publishes an answer and sets the mode to `Normal`, the
queue head can remain stale until a later implementation/test catches
it. Add either an explicit `CONFIRM_REPLY_TOPIC` subscription/prune in
cK3, or an explicit `pop_head`/equivalent in cK4, plus a row-local
acceptance assertion that the head is removed after the own-answer path.

## Non-blocking

### N1 — cK2/cK3 exceed 100 LoC without an explicit size justification

cK2 and cK3 each list `~120 lines net`. The amendment's size rubric is
`≤5 files / ≤100 LoC OR explicit justification`; these rows should add a
short body-justification for the over-100-LoC estimates, similar to the
existing cK5/cK6 justifications.

## Checks that passed

- `rfl_tui.rs:309-360` is the live `run_production_mode` / `ui_loop`
  range; `rfl_tui.rs:83-85` is the `cfg.test_message` gate;
  `publish_submitted_line` starts at `rfl_tui.rs:235`.
- cK5 is correctly scoped to production mode: it explicitly does not set
  `RFL_TUI_TEST_MESSAGE`, `RFL_TUI_TEST_CONFIRM_ANSWER`, or
  `RFL_TUI_TEST_CONFIRM_ANSWERS` for the asserted body and drives the
  overlay answer with `tmux send-keys "a"`.
- Dependency lines name earlier rows / preexisting milestone surfaces;
  no forward dependency found inside Phase K.
- `transcripts/section-5-phase-k/` is a new sibling of the existing
  `transcripts/section-5/` path and does not collide with c27's files.
