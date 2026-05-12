# m6.1 commits.md — pi review round 3

Verdict: BLOCKING

Counts: B/1 M/0 N/1

## Blockers

### B-1: c02's existing-test update list still misses `rfl_init_without_lock_not_yet_implemented.rs`

The pi-2 B-1 fold is directionally right: c02 now recognizes that existing successful `rfl init` subprocess tests need an explicit `RFL_BUNDLED_BIN_OPENAI` once `init::run` stops relying on shim bytes and calls `resolve_runtime_binary(&OPENAI_NAMES)`. But the affected-test list is incomplete.

`rafaello/crates/rafaello/tests/rfl_init_without_lock_not_yet_implemented.rs` also reaches `pp1::materialise`:

- it builds a synthetic bundled `openai/` fixture with the shim (`rfl_init_without_lock_not_yet_implemented.rs:14-40`),
- calls `workspace_bin("rfl")` only (`:42`),
- runs `rfl init --yes --project-root ...` (`:43-48`),
- sets `RFL_BUNDLED_PLUGINS_DIR` but not `RFL_BUNDLED_BIN_OPENAI` (`:48`),
- and asserts success + lock presence (`:52-57`).

That is the same class as the seven tests named in commits.md lines 25-31 / c02 lines 400-407. After c02, this test will be order-dependent on whether `target/<profile>/rfl-openai` happens to exist, because it does not call `workspace_bin("rfl-openai")` and does not set the explicit override.

Concrete fix:

- Add `rfl_init_without_lock_not_yet_implemented.rs` to the round-3 banner and c02 step 6 affected-test list.
- Update c02 sizing from 9 files / 7 existing tests to 10 files / 8 existing tests.
- Update the cross-check section to say the listed materialising tests include this file.

On the c02 split question: I accept the driver's single-commit cohesion argument **once this missing test is added**. The production change, C1 acceptance, and one-line updates to existing materialising tests must land together to keep the suite green. Splitting c02a/c02b would create an intermediate commit with known failing tests or force artificial staging. The 10-file count is above the usual guideline, but the extra 8 files are one-line test plumbing directly caused by the same behavioral change.

## Majors

None.

## Nits

### N-1: Sizing summary still says all commits respect the ≤5-file guideline while c02 is above it

commits.md correctly admits in c02's size paragraph that c02 is above the CLAUDE.md ≤5-file guideline (lines 457-468), but the global sizing summary still says “All commits respect the CLAUDE.md ≤5-file / ≤100-line production-code envelope” (lines 831-834). With c02 already at 9 files in the draft — and 10 after B-1 is fixed — that sentence should be reworded.

Suggested wording: “All commits keep production-code deltas within the intended envelope; c02 intentionally exceeds the file-count guideline only for one-line updates to existing tests needed to keep the suite green.”

## Items the scope handles correctly (confirmation)

- pi-2 N-1 is folded: the round-3 changelog now correctly excludes c04 from the `-p rafaello --test <name>` acceptance-command shape and calls out the `rafaello-tui --bin rfl-tui` command (commits.md:53-60). The c04 row itself uses the appropriate command for a bin `#[cfg(test)]` module.
- pi-2 N-2 is folded: the traceability appendix now maps §D to `manual-validation.md` + `00-CONTEXT.md` + 3 transcripts (commits.md:764-780).
- The c02 cross-check no longer makes the incorrect `CARGO_BIN_EXE_*` claim; it now correctly says the new resolver deliberately does not consult that env and that tests reaching materialisation need `RFL_BUNDLED_BIN_OPENAI` (commits.md:784-802). It just needs the missing file added.
- The c01 test seam remains acceptable and does not change the public `resolve_plugin_dir(name)` signature used by `install.rs:96`.
- The c05 `ulid` dev-dependency and Linux cfg gate remain correctly folded.

## Out-of-scope checks performed (negative coverage)

- Re-grepped the `rfl_init*.rs` tests for `--yes` / `--force` patterns and checked short-circuit categories. Decline/EOF/idempotent/help do not reach `pp1::materialise`; `rfl_init_without_lock_not_yet_implemented.rs` does and is the missing case.
- Re-checked the c02a/c02b split option. I do not request a split; a single c02 is acceptable after correcting the affected-test list and sizing text.
- No new issue found in the c04 acceptance-command fold or the c06 provenance traceability fold.
