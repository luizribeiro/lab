# m6.1 commits.md — pi review round 1

Verdict: BLOCKING

Counts: B/2 M/4 N/3

## Blockers

### B-1: c05 uses `ulid::Ulid` in the `rafaello` test crate without adding a direct dependency

c05 line 424-426 specifies `format!("rfl-c05-ctrlc-{}", ulid::Ulid::new())` and says the `ulid` crate is already in the workspace. It is in the workspace dependency table (`rafaello/Cargo.toml:38`) and is used by `rafaello-tui` (`rafaello/crates/rafaello-tui/Cargo.toml:29`, `rfl_tui.rs` imports `ulid::Ulid`), but the new test is in the `rafaello` crate, whose `Cargo.toml` currently has no `ulid` normal or dev dependency (`rafaello/crates/rafaello/Cargo.toml:15-33`). Workspace dependencies are not automatically available to every member crate.

As written, `rfl_chat_ctrl_c_quits_cleanly.rs` will not compile when it imports `ulid::Ulid`.

Concrete fix: c05 must either:

- add `ulid = { workspace = true }` under `rafaello/crates/rafaello/Cargo.toml` `[dev-dependencies]` and update c05 size/files touched accordingly; or
- avoid `ulid` in the test and use an already-available uniqueness source. The scope, however, ratified `Ulid::new()` as the nonce, so adding the dev-dependency is the cleaner fix.

Also update the scope/commits “named files”/size text if needed so the per-commit agent is not surprised by the Cargo.toml touch.

### B-2: c05 omits the cK5 Linux-only gate

c05 says it mirrors cK5 (`rfl_chat_production_tui_input_overlay_e2e.rs`) but only carries the `tmux -V` skip (commits.md:403-408). The cK5 precedent is explicitly Linux-only via `#![cfg(target_os = "linux")]` at the top of the file, before any test code. That is load-bearing: these `rfl chat` integration tests exercise the lockin/syd/PTY path and should not accidentally run on macOS just because `tmux` happens to be installed.

Without the target cfg, a macOS developer/CI host with tmux could run this new test and fail in the sandbox/spawn path rather than cleanly skipping like cK5.

Concrete fix: c05 should specify `#![cfg(target_os = "linux")]` at the top of `rfl_chat_ctrl_c_quits_cleanly.rs`, matching cK5. The acceptance command can remain the Linux demo-bar command, but the test file itself must be target-gated.

## Majors

### M-1: c01's unit-test plan needs a test seam or stricter serialization/cleanup around `current_exe()`-relative release paths

c01's production functions use `std::env::current_exe()` (matching current `bundled.rs:27-36`) and the acceptance tests propose setting up temp release hierarchies at `<exe-parent>/../share/...` (commits.md:132-162). Because the functions do not accept an injectable `current_exe`/parent path, those tests will have to create directories under the real test binary's target tree (typically `target/debug/share/...`). That is workable, but it is global mutable test state, not a tempdir-owned hierarchy.

The row partially addresses env pollution with `serial_test`/guards (lines 180-183), but it should be more explicit that all tests mutating `RFL_BUNDLED_*` or `<current_exe-parent>/../share/...` run under the same serial key and remove their created release-tree directories on drop. The `resolve_runtime_binary_not_found_lists_all_arms` wording at lines 168-173 also says “no workspace,” which is not true for tests whose `current_exe` lives under the workspace target dir; the test should use a fake `BundledPluginNames` with a missing `runtime_bin` to force NotFound after the workspace is found.

Concrete fix: either add small private `*_from_exe_parent(...)` helper functions so unit tests can pass a temp exe parent, or explicitly require a shared serial key + cleanup guard and adjust the NotFound test wording to “workspace exists but runtime bin is absent.”

### M-2: c02's C1 SHA-256 assertion needs either a direct dev-dependency or a no-new-dep byte comparison

c02 acceptance lines 258-260 require computing SHA-256 of the materialised file and of `rfl-openai-stub`. The `rafaello` crate currently does not depend on `sha2` or `data-encoding` directly (`rafaello/crates/rafaello/Cargo.toml:15-33`), even though `sha2` is present in the workspace dependency table. A test in the `rafaello` crate cannot import `sha2` unless c02 also edits `rafaello/crates/rafaello/Cargo.toml`.

The test does not need hashing for correctness; exact bytes can be asserted by reading both files and comparing the byte vectors. That also better matches the scope's “byte-identical” intent without adding another touched file.

Concrete fix: change c02 acceptance to “read both files and assert byte equality,” or explicitly add `sha2 = { workspace = true }` (and perhaps `data-encoding`) as `rafaello` dev-dependencies and update the size/files list. I prefer raw byte equality.

### M-3: c06 transcripts need provenance metadata if they are ratification evidence

c06 lines 494-516 describe a reproducible recording method and “documentation-grade evidence,” but the transcript files themselves are host-sensitive (tmux version, terminal width/height, rfl binary build path, date, environment). m6's retrospective had to distinguish authentic captures from schematic ones, so v0.1.1 should include enough provenance to make these captures auditable.

Concrete fix: require `manual-validation.md` or `transcripts/v0_1_1/00-CONTEXT.md` to record at least date, hostname/user or worktree path, `git rev-parse HEAD`, `tmux -V`, terminal size, exact `cargo build`/`rfl` path used, and whether `CARGO_BIN_EXE_*`/`RFL_BUNDLED_BIN_OPENAI` were absent. The transcript contents can remain illustrative/plain text; the provenance header makes them useful ratification evidence.

### M-4: c03/c05 acceptance commands drift from the scope demo-bar command

The scope demo bar runs `cargo test --manifest-path rafaello/Cargo.toml --workspace --features rafaello-core/test-fixture -p rafaello --test ...`. c03 and c05 acceptance commands omit `--workspace --features rafaello-core/test-fixture` (commits.md:341-345 and 462-465). They may still pass locally because the test helpers call `cargo build --workspace --bins --features rafaello-core/test-fixture`, but the per-commit acceptance commands should match the ratified demo bar unless there is a reason to narrow them.

Concrete fix: update c03 and c05 acceptance commands to the exact scope demo-bar shape, or explicitly state why the narrower command is sufficient.

## Nits

### N-1: c03 should choose one topic-id lookup path

c03 lines 318-321 say to locate the materialised entry by either `topic_id::derive("builtin:openai@0.0.0")` or by reading the lock. `rafaello_core::topic_id::derive` is public (`rafaello-core/src/topic_id.rs:15`), and the canonical id is fixed by init. Prefer the direct derive path; it is simpler and avoids making the test's path lookup depend on successfully parsing the same lock whose digest it later checks.

### N-2: c04's import instructions are redundant

c04's snippet includes `use crossterm::event::KeyModifiers;` inside the function (commits.md:356-362) and then separately says to add `KeyModifiers` to the existing top-of-file import list (lines 364-365). Do only the top-level import; otherwise the row invites a redundant local import.

### N-3: c01 test count/sizing is defensible but the row should stop apologizing about a split

The c01 split is cohesive: `BundledPluginNames`, the source-tree sister resolver, the runtime resolver, and their unit tests form one API surface. Splitting would either land untested helper code or force artificial test duplication. The size is acceptable because production code remains small and tests are the bulk. The “acceptable to split tests into a second commit if needed” wording at lines 189-191 weakens the plan and could cause an agent to separate tests from logic, contrary to the cited commit guideline. Reword to affirm that c01 is intentionally one code+tests commit.

## Items the scope handles correctly (confirmation)

- Scope traceability is mostly complete. §A0/§A1 map to c01; §A2/§A3/C1 map to c02; §C2 maps to c03; §B1/§B4/C3 map to c04; §C4 maps to c05; §D maps to c06.
- c01 keeping `resolve_plugin_dir(name)` unchanged is the right way to preserve `rfl install` (`install.rs:95-103`) while giving `rfl init` a release-dir-aware sister function.
- c01 does not need to split on conceptual grounds; the struct and both resolvers are one `bundled`-module surface, and tests should land with the logic.
- c02's digest ordering is correct: current `init.rs` computes `content_digest(&target_dir)` immediately after `pp1::materialise` (`init.rs:104-112`) and no other consumer uses it before it is placed into `PluginEntry`. Moving it after copy/chmod is the required fix.
- c02's failure cleanup is scoped correctly to `resolve_runtime_binary` `NotFound`, matching scope §A3. `pp1::materialise` removes any previous target dir before copying (`pp1.rs`), so the cleanup removes only this run's just-materialised shim tree.
- c03's no-override dev-fallback chain is valid under default target layout: `workspace_bin("rfl")` builds `target/debug/rfl`; walking up from `target/debug` reaches the workspace root; `workspace_bin("rfl-openai")` ensures `target/debug/rfl-openai` exists.
- c04's `KeyEvent` fields are valid for crossterm 0.28 (`rafaello/Cargo.toml` pins `crossterm = "0.28"`), including `state: KeyEventState::NONE`. Placing the Ctrl-C guard after the Release filter and before mode dispatch matches scope §B1.
- c05's `InstallOptions { provider_executable, tool_executable, real_binaries }` matches `tests/common/m4_install.rs:23-34`.
- c05 copying cK5 helpers locally is acceptable for this patch milestone. A shared `tests/common/tmux_helpers.rs` would be cleaner long-term, but refactoring cK5 mid-patch would expand the surface; duplicated small helpers are fine here.
- The dependency graph is honest: c04 is technically independent of c01-c03 but sequenced after them only to keep D1 contiguous; c05 depends on c04; c06 depends on the fixes being present for authentic captures.

## Out-of-scope checks performed (negative coverage)

- Checked the ratified owner decisions from scope: directory name, C2 lock-correctness depth, and explicit `RFL_BUNDLED_BIN_OPENAI` override are preserved.
- Checked PP1 containment: the plan overwrites bytes in-place and does not introduce symlink/out-of-package entry paths.
- Checked that c05 avoids parent-side signal-forwarding changes; it tests the child raw-mode key path only.
- Checked workspace dependencies for `tempfile`, `serial_test`, `crossterm`, and `ulid`; only `ulid` is missing from the `rafaello` crate itself.
