# m6.1 commits.md — pi review round 2

Verdict: BLOCKING

Counts: B/1 M/0 N/2

## Blockers

### B-1: c02 does not account for existing `rfl init` tests that will now need a runtime-binary source

The pi-1 folds are mostly correct, but c02 still misses a regression in the pre-existing init test suite. After c02, every successful `rfl init --yes` path reaches the new `resolve_runtime_binary(&OPENAI_NAMES)` call after `pp1::materialise`. Existing init tests in `rafaello/crates/rafaello/tests/rfl_init_*.rs` already spawn `rfl init` with `RFL_BUNDLED_PLUGINS_DIR` pointing at a synthetic `openai/` fixture, but they do **not** set `RFL_BUNDLED_BIN_OPENAI`. Examples include `rfl_init_writes_default_lock.rs`, `rfl_init_materialises_package_dir.rs`, `rfl_init_force_rewrites.rs`, `rfl_init_idempotent_no_overwrite.rs`, `rfl_init_then_install_against_in_tree_bundled_smoke.rs`, and several decline/EOF tests.

commits.md's cross-check says these tests remain green because they run under cargo with `CARGO_BIN_EXE_*` available (lines 540-553). That is not sufficient for the ratified resolver: §A1/c01 intentionally does **not** consult `CARGO_BIN_EXE_rfl-openai`; it consults `RFL_BUNDLED_BIN_OPENAI`, the release tree, or `<workspace>/target/<profile>/rfl-openai`. Many existing tests only call `workspace_bin("rfl")`; if `target/debug/rfl` already exists but `target/debug/rfl-openai` does not, `workspace_bin("rfl")` will not invoke the all-bins build, and the new runtime resolver can fail with `BundledError::NotFound`. That makes the existing suite dependent on target-dir incidental state.

This is material because c02 changes `init::run`, not only the new C1 test. The commit plan must explicitly preserve the existing init tests, not rely on a lucky clean target.

Concrete fix options:

1. In c02, update the existing `rfl_init_*` tests that exercise the successful materialisation path to set `RFL_BUNDLED_BIN_OPENAI` to a workspace-built runtime (stub or real `rfl-openai`) before spawning `rfl init`. Prefer a shared test helper if possible, but the commits plan must account for the extra files/size if many tests need edits.
2. Alternatively, add a small test-only helper used by those tests that ensures `workspace_bin("rfl-openai")` exists before spawning `rfl init`; this preserves the no-override dev fallback in existing tests but still needs explicit test-row coverage and avoids order-dependent target state.
3. Correct the Cross-checks section: remove the claim that `CARGO_BIN_EXE_*` makes the c21 tests safe, because the new resolver deliberately does not use that ambient leak.

Do **not** add `CARGO_BIN_EXE_rfl-openai` as a production fallback to make these tests pass; that would undercut the scope's explicit-env/no-cargo-leak design and the C2 regression.

## Majors

None.

## Nits

### N-1: round-2 changelog overclaims c04's acceptance command shape

The round-2 banner says c02/c03/c04/c05 acceptance commands all match `cargo ... -p rafaello --test <name>` exactly. c04 correctly cannot use that shape because its test lives in the `rafaello-tui` binary unit-test module; the c04 row uses `-p rafaello-tui --bin rfl-tui -- handle_terminal_event`, which is the right command. Fix the banner/changelog wording to exclude c04 from the `-p rafaello --test <name>` claim or state that c04 uses the analogous bin-test command.

### N-2: §D traceability should mention `00-CONTEXT.md`

c06 now correctly adds `transcripts/v0_1_1/00-CONTEXT.md` provenance (lines 479-503), but the traceability appendix still says §D maps to `manual-validation.md` + 3 transcripts. Include `00-CONTEXT.md` there so the provenance file remains part of the ratified evidence set.

## Items the scope handles correctly (confirmation)

- pi-1 B-1 is folded: c05 adds `ulid = { workspace = true }` to `rafaello/crates/rafaello/Cargo.toml` dev-dependencies and updates the size/files touched text (c05 lines 535-541 and 600-604).
- pi-1 B-2 is folded: c05 now specifies `#![cfg(target_os = "linux")]` at file top (lines 523-528), matching cK5's Linux-only gate.
- pi-1 M-1 is folded acceptably: c01 introduces private `_from_exe_parent` seams so resolver tests can use temp exe-parent trees instead of mutating the real target dir, while preserving the public `resolve_plugin_dir(name)` signature for `install.rs:96` (c01 lines 95-105, 193-234).
- pi-1 M-2 is folded: c02 uses raw `Vec<u8>` byte equality instead of SHA-256 and therefore needs no new `sha2`/encoding dependency (lines 337-340).
- pi-1 M-3 is folded: c06 requires `00-CONTEXT.md` with date, host/worktree, git HEAD, tmux version, terminal size, exact build/rfl path, and relevant env presence (lines 479-503). That is enough provenance without over-specifying transcript contents.
- pi-1 N-1/N-2/N-3 are folded in the commit rows: c03 commits to `topic_id::derive("builtin:openai@0.0.0")`; c04 removes the redundant inline import; c01 size wording now affirms code+tests in one cohesive commit.
- c05's copy-rather-than-refactor choice for tmux helpers remains appropriate for this patch milestone; factoring cK5 into shared helpers would broaden the surface unnecessarily.
- c01's cohesive size remains acceptable: production code is small, and the larger line count is mostly in unit tests that belong with the resolver API.

## Out-of-scope checks performed (negative coverage)

- Re-checked the `resolve_plugin_dir` preservation path against `install.rs:95-103`; round-2's private seam does not require changing the install call site.
- Re-checked c05's dependency and target-gating folds against workspace `Cargo.toml` and cK5; no remaining issue there.
- Re-checked c06 provenance fields for overspec/underspec; the only residual is the traceability-table mention in N-2.
- Re-checked c03's no-override fallback chain under default target layout; it remains sound once `workspace_bin("rfl-openai")` has been called in that test.
