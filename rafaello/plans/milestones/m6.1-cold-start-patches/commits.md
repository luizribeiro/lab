# m6.1 — v0.1.1 cold-start patches — commits

> **Status:** round 3 — claude-authored 2026-05-12, awaiting
> pi round 3. Folds `commits-pi-review-2.md` (1B / 0M / 2N,
> BLOCKING) on top of round 2.
>
> **Round-3 changelog (every pi-2 finding folded):**
>
> - **B-1 (existing `rfl init` tests will fail after c02).**
>   The pre-existing `rfl_init_*` tests set
>   `RFL_BUNDLED_PLUGINS_DIR` but **not**
>   `RFL_BUNDLED_BIN_OPENAI`. After c02 makes `init::run`
>   call `resolve_runtime_binary(&OPENAI_NAMES)`, the dev
>   fallback walks up to
>   `<workspace>/target/<profile>/rfl-openai`. That binary
>   only exists if a prior cargo build produced it, so the
>   existing tests would be order-dependent on target-dir
>   state. **Round-3 fix**: c02 explicitly adds
>   `.env("RFL_BUNDLED_BIN_OPENAI",
>   workspace_bin("rfl-openai-stub"))` to every existing
>   `rfl_init_*` test that reaches `pp1::materialise`. The
>   affected tests (verified via grep for `--yes` /
>   `--force` and absence of decline/short-circuit
>   patterns):
>   - `rfl_init_writes_default_lock.rs`
>   - `rfl_init_yes_skips_prompt.rs`
>   - `rfl_init_round_trip_byte_stable.rs`
>   - `rfl_init_materialises_package_dir.rs`
>   - `rfl_init_writes_lock_against_synthetic_bundled_tree.rs`
>   - `rfl_init_force_rewrites.rs`
>   - `rfl_init_then_install_against_in_tree_bundled_smoke.rs`
>   Tests that short-circuit before `pp1::materialise`
>   (decline, EOF, idempotent-no-overwrite, --help) are
>   **not** updated — the resolver is never reached on
>   those paths. Each affected test gains exactly **one
>   line** (`.env("RFL_BUNDLED_BIN_OPENAI",
>   workspace_bin("rfl-openai-stub"))`) before
>   `.output()`/`.spawn()`. Cumulative diff: 1 production
>   file + 1 new C1 test + 7 existing tests × 1 line =
>   9 files, ~160 lines total. Defended as one cohesive
>   commit per CLAUDE.md "Tests and Business Logic: Same
>   Commit, Always" — the production change and the test
>   plumbing must ship together to keep the suite green.
>   If pi prefers a split, c02 splits into:
>   - c02a (`init.rs` + C1 acceptance), 2 files.
>   - c02b ("update existing tests to declare runtime-bin
>     env"), 7 files / 7 lines.
>   Round-3 lean is **one commit (c02)**; pi adjudicates.
>   The cross-checks section is also corrected to drop the
>   incorrect "`CARGO_BIN_EXE_*` makes c21 tests safe"
>   claim — the new resolver deliberately does not
>   consult that env var.
> - **N-1 (round-2 banner overclaimed c04 acceptance
>   command shape).** c04's test lives in the
>   `rafaello-tui` binary's `#[cfg(test)]` module, so its
>   acceptance command uses `-p rafaello-tui --bin
>   rfl-tui -- handle_terminal_event`, not `-p rafaello
>   --test <name>`. Round-3 banner / changelog wording
>   updated to call this out and exclude c04 from the
>   blanket claim.
> - **N-2 (traceability appendix missing
>   `00-CONTEXT.md`).** Round-3 appendix lists
>   `00-CONTEXT.md` alongside the appendix file + 3
>   transcripts for §D coverage.
>
> Cumulative trajectory: round 1 → 2B/4M/3N (BLOCKING) →
> round 2 → 1B/0M/2N (BLOCKING) → round 3 (this commit),
> target verdict CONVERGED.
>
> ---
>
> **(History — round 2 status, preserved for traceability.)**
>
> Round 2 — claude-authored 2026-05-12. Folds
> `commits-pi-review-1.md` (2B / 4M / 3N, BLOCKING) on top
> of round 1.
>
> **Round-2 changelog (every pi-1 finding folded):**
>
> - **B-1 (c05 missing `ulid` dev-dep).** `ulid` is a
>   workspace-level dep but not a direct dep of the
>   `rafaello` crate. Round-2 c05 explicitly adds
>   `ulid = { workspace = true }` to
>   `rafaello/crates/rafaello/Cargo.toml`
>   `[dev-dependencies]` and updates the c05 files-touched
>   count.
> - **B-2 (c05 missing Linux-only cfg gate).** cK5 carries
>   `#![cfg(target_os = "linux")]` at file top because the
>   lockin/syd/PTY stack is Linux-only. Round-2 c05 adds
>   the same cfg gate to `rfl_chat_ctrl_c_quits_cleanly.rs`.
> - **M-1 (c01 test seam for `current_exe()`-relative
>   release paths).** Round-2 c01 introduces private
>   `*_from_exe_parent(parent: &Path, names: &BundledPluginNames)`
>   helper functions that the public `resolve_*` fns
>   delegate to with `std::env::current_exe()?.parent()`.
>   The unit tests target the `_from_exe_parent` seam with
>   a `tempfile::tempdir()`-rooted exe-parent, avoiding any
>   mutation of the actual test binary's target tree. The
>   `not_found_lists_all_arms` test wording is also fixed
>   per pi-1 M-1 (use a `BundledPluginNames` with a name
>   whose `runtime_bin` is absent under the temp exe parent
>   — workspace exists in the dev-fallback sense but the
>   binary is absent).
> - **M-2 (c02 SHA-256 introduces a new dep).** Round-2 c02
>   acceptance switches from SHA-256 hashing to raw
>   `Vec<u8>` byte equality. No new dep needed. Equivalent
>   coverage; simpler test.
> - **M-3 (c06 provenance).** Round-2 c06 adds a
>   `transcripts/v0_1_1/00-CONTEXT.md` provenance file with
>   the required metadata (date, hostname or worktree path,
>   `git rev-parse HEAD`, `tmux -V`, terminal width × height,
>   exact `cargo build` / `rfl` path used, whether
>   `CARGO_BIN_EXE_*` / `RFL_BUNDLED_BIN_OPENAI` were
>   present). Transcripts themselves remain plain text.
> - **M-4 (acceptance commands drift).** Round-2 c02 / c03 /
>   c04 / c05 acceptance commands match the scope demo-bar
>   shape exactly:
>   `cargo test --manifest-path rafaello/Cargo.toml
>   --workspace --features rafaello-core/test-fixture -p
>   rafaello --test <name>`. The c01 acceptance (in-file
>   unit tests) remains `cargo test --manifest-path
>   rafaello/Cargo.toml --workspace --features
>   rafaello-core/test-fixture -p rafaello bundled::`.
> - **N-1 (c03 topic-id lookup).** Round-2 c03 commits to
>   `topic_id::derive("builtin:openai@0.0.0")` directly
>   (not lock-reading). Simpler and decouples the path
>   lookup from the lock-parse step that the same test
>   later verifies.
> - **N-2 (c04 redundant import).** Round-2 c04 specifies
>   only the top-of-file import update; the inline `use
>   crossterm::event::KeyModifiers;` is removed from the
>   snippet.
> - **N-3 (c01 split apology).** Round-2 c01 size wording
>   affirms the bundled code+tests shape and removes the
>   "acceptable to split tests" hedge.
>
> Cumulative trajectory: round 1 → 2B/4M/3N (BLOCKING) →
> round 2 (this commit), target verdict CONVERGED or
> NON-BLOCKING.
>
> ---
>
> **(History — round 1 status, preserved for traceability.)**
>
> Round 1 — claude-authored 2026-05-12. Built on top of
> `scope.md` round-5 CONVERGED (`6912344`, owner-ratified).
> Six commits, sized 5–10 per the owner directive.

## Reading order for per-commit agents

Each per-commit agent gets (inlined into its prompt, not
cited by row number — see plans/README.md §m1 4.2):

1. The full commit-row text below for its commit.
2. `scope.md` (the round-5 CONVERGED draft).
3. The relevant sections of `overview.md` (§8.1, §16),
   `decisions.md` (rows 38, 59–68), and `glossary.md`.
4. The specific code surfaces the row points at, with
   file:line citations.

Per-commit agents work in fresh worktrees under
`/home/luiz/lab-wt/v0.1.1-c<NN>/` on branches
`agents/v0.1.1/c<NN>`. Pre-commit symlink is applied at
worktree creation. The driver ff-merges each commit to
`agents/v0.1.1/driver` in order, then `rm -rf
rafaello/target` in the agent worktree (disk hygiene per
m4–m6 retros) before deleting the worktree.

## Phase ordering rationale

Linear sequence, no phase split needed at this size. Order
chosen so each commit lands a self-contained logical unit:

- **c01** ships the `bundled` helpers (struct + sister
  function + runtime resolver). Lands first because c02's
  init swap and both subprocess regression tests depend on
  the helpers existing.
- **c02** ships the `init::run` shim → real-binary swap +
  in-process exact-bytes acceptance (C1). Self-contained
  given c01.
- **c03** ships the subprocess no-override regression test
  (C2). Independent of c02's code path internally but the
  test asserts the c02 contract, so it lands after.
- **c04** ships the rfl-tui Ctrl-C handler fix + unit test
  (C3). Independent from D1; could land before c01 but
  grouped after to keep D1 commits contiguous.
- **c05** ships the cK5-style tmux Ctrl-C regression
  (C4). Depends on c04's handler fix.
- **c06** ships the manual-validation appendix transcripts.
  Last because it captures the post-fix behaviour.

Retrospective drift commits (decisions.md row append,
optional glossary.md entry) land at retrospective time, not
in this commits.md table — same pattern as m4/m5/m6.

## Commit table

### c01 — feat(rafaello): `BundledPluginNames` + sister fn + `resolve_runtime_binary`

- **What.** Three additions in
  `rafaello/crates/rafaello/src/bundled.rs`. None modify the
  existing `resolve_plugin_dir(name)` signature or
  behaviour.

  **Test seam (round 2).** The two new public resolvers
  are thin wrappers around private
  `*_from_exe_parent(parent: &Path, names: &BundledPluginNames)`
  helpers. The public fns do
  `std::env::current_exe()?.parent()` and delegate to the
  helper; the private helpers do all the actual joining /
  env reads / walk-up work. Unit tests call the
  `_from_exe_parent` seam with a
  `tempfile::tempdir()`-rooted exe parent, so no test
  mutates the real `<workspace>/target/...` tree (pi-1
  M-1). The walk-up dev fallback inside
  `_from_exe_parent` still calls `parent.ancestors()`
  unchanged.
  1. New `pub struct BundledPluginNames { pub dev_crate:
     &'static str, pub release_dir: &'static str, pub
     runtime_bin: &'static str }` plus `pub const
     OPENAI_NAMES: BundledPluginNames = BundledPluginNames
     { dev_crate: "openai", release_dir: "rfl-openai",
     runtime_bin: "rfl-openai" };`. Field doc-comments
     state the consumer per scope §A0:
     - `dev_crate`: workspace walk-up to
       `crates/rafaello-<dev_crate>/`; also the
       `RFL_BUNDLED_PLUGINS_DIR` env-arm join axis; also
       the `RFL_BUNDLED_BIN_<NAME_UPPER>` munge source.
     - `release_dir`: release-arm join under
       `$out/share/rafaello/plugins/<release_dir>/`.
     - `runtime_bin`: release runtime path
       `<release_dir>/bin/<runtime_bin>` and dev-target
       `<workspace>/target/<profile>/<runtime_bin>`.
  2. New `pub fn
     resolve_plugin_dir_for_bundled(names:
     &BundledPluginNames) -> Result<PathBuf, BundledError>`.
     Three-arm resolution per scope §A0 step 2:
     - **env arm**:
       `<RFL_BUNDLED_PLUGINS_DIR>/<names.dev_crate>` (the
       `RFL_BUNDLED_PLUGINS_DIR` env var is read with
       `std::env::var_os`; if set, the join is attempted
       and returned iff `.is_dir()`).
     - **release arm**: `<rfl-exe-parent>/../share/rafaello/plugins/<names.release_dir>/`.
     - **dev fallback**: walk up from `<rfl-exe-parent>`
       looking for a workspace `Cargo.toml` containing
       `[workspace]`; if found, return
       `<workspace>/crates/rafaello-<names.dev_crate>/`.
     - None → `BundledError::NotFound { name:
       names.dev_crate.to_owned() }` with a message naming
       all three tried locations.
  3. New `pub fn resolve_runtime_binary(names:
     &BundledPluginNames) -> Result<PathBuf, BundledError>`
     per scope §A1. Resolution order:
     - **env override**: `RFL_BUNDLED_BIN_<NAME_UPPER>`
       where `<NAME_UPPER>` is `names.dev_crate` with
       hyphens → underscores, uppercased (e.g. `openai` →
       `RFL_BUNDLED_BIN_OPENAI`). Value must be an
       absolute path to a regular file with the user-exec
       bit set, else `NotFound`.
     - **release arm**:
       `<rfl-exe-parent>/../share/rafaello/plugins/<names.release_dir>/bin/<names.runtime_bin>`.
     - **dev fallback**: walk up from `<rfl-exe-parent>`
       looking for `[workspace]` `Cargo.toml`; if found,
       return
       `<workspace>/target/<profile>/<names.runtime_bin>`
       where `<profile>` is `debug` in
       `cfg!(debug_assertions)` else `release`.
     - None → `BundledError::NotFound { name:
       names.runtime_bin.to_owned() }` with a message
       naming all three tried locations (env var literal
       resolved at runtime so `openai` shows
       `RFL_BUNDLED_BIN_OPENAI` per pi-1 N-1).
- **Why.** Scope §A0 (`resolve_plugin_dir_for_bundled` to
  fix the latent release-layout bug without touching `rfl
  install`'s `resolve_plugin_dir(name)` call site at
  `install.rs:96`) and scope §A1 (runtime-binary resolver
  for the init swap). Bundling these three additions into
  one commit keeps the new `bundled`-module surface
  cohesive (one struct, two sister resolvers, all keyed off
  `BundledPluginNames`). c02's init swap and the subprocess
  regression test (c03) both depend on these.
- **Depends on.** `66be0fd` (scope.md round-5 CONVERGED tip)
  + owner-ratification commit.
- **Acceptance.** Unit tests in
  `rafaello/crates/rafaello/src/bundled.rs`'s `#[cfg(test)]`
  module. Every test below targets the **`_from_exe_parent`
  seam** with a `tempfile::tempdir()`-rooted `parent`
  argument; no test touches `std::env::current_exe()` or
  the real workspace target tree.
  1. `resolve_plugin_dir_release_arm_uses_input_name_verbatim`
     — the pre-existing `resolve_plugin_dir(name)` (its
     own `_from_exe_parent` seam, factored in this commit
     for symmetry). Build a temp exe-parent with
     `<parent>/../share/rafaello/plugins/rfl-mailcat/`
     present and `<parent>/../share/rafaello/plugins/rfl-rfl-mailcat/`
     absent; `RFL_BUNDLED_PLUGINS_DIR` unset. Assert the
     function returns the former (regression guard —
     proves m6.1 did not break `rfl install`).
  2. `resolve_plugin_dir_for_bundled_env_arm_hit` — set
     `RFL_BUNDLED_PLUGINS_DIR=<tmp>` and create
     `<tmp>/openai/rafaello.toml`. Assert
     `resolve_plugin_dir_for_bundled_from_exe_parent(
     &temp_parent, &OPENAI_NAMES)` returns `<tmp>/openai`.
  3. `resolve_plugin_dir_for_bundled_release_arm_hit` —
     `RFL_BUNDLED_PLUGINS_DIR` unset; set up a temp
     hierarchy with `<temp-parent>/../share/rafaello/plugins/rfl-openai/`
     present. Assert the function returns that directory.
  4. `resolve_plugin_dir_for_bundled_dev_fallback_hit` —
     no env, no release tree; build a temp exe parent
     sitting underneath a temp synthetic workspace
     (`<tmp-workspace>/Cargo.toml` containing
     `[workspace]`, plus
     `<tmp-workspace>/crates/rafaello-openai/`). Assert
     the function walks up to the workspace and returns
     `<tmp-workspace>/crates/rafaello-openai/`.
  5. `resolve_runtime_binary_env_override_hit` —
     `RFL_BUNDLED_BIN_OPENAI=<tmp>/sentinel` where
     `<tmp>/sentinel` is a 0o755 regular file; assert the
     resolver returns that path. Negative variant: same
     path but mode 0o644 → `NotFound`.
  6. `resolve_runtime_binary_release_arm_hit` — set up a
     temp `<parent>/../share/rafaello/plugins/rfl-openai/bin/rfl-openai`
     0o755 file; no envs set; assert the resolver returns
     that path.
  7. `resolve_runtime_binary_dev_fallback_hit` — synthetic
     workspace at `<tmp-workspace>/` with
     `<tmp-workspace>/target/debug/rfl-openai` 0o755;
     temp exe parent under `<tmp-workspace>/target/debug/`;
     no envs. Assert the resolver returns the synthetic
     binary path.
  8. `resolve_runtime_binary_not_found_lists_all_arms` —
     synthetic workspace exists but **the runtime binary
     is absent** under
     `<tmp-workspace>/target/debug/rfl-openai`; no
     env; no release tree. Assert
     `BundledError::NotFound`. Inspect the `Display` impl
     and assert it contains the literal
     `RFL_BUNDLED_BIN_OPENAI`, the release-arm path
     substring, and the dev-fallback substring.
  9. `resolve_runtime_binary_env_var_name_munge` — call
     `resolve_runtime_binary_from_exe_parent` with a
     `BundledPluginNames { dev_crate: "foo-bar", .. }`;
     ensure the env-var lookup is
     `RFL_BUNDLED_BIN_FOO_BAR` (not
     `RFL_BUNDLED_BIN_foo-bar`) — set the expected env var
     and observe the override hit.
  - All tests run under a single `serial_test::serial`
    key (`bundled_env`) because they mutate process-global
    env vars (`RFL_BUNDLED_PLUGINS_DIR`,
    `RFL_BUNDLED_BIN_OPENAI`, `RFL_BUNDLED_BIN_FOO_BAR`).
    `serial_test` is already a workspace dev-dep
    (verify via `rafaello/Cargo.toml`). Each test owns a
    small guard struct (RAII `EnvGuard`) that
    `remove_var`s anything it set on Drop.
- **Size.** 1 file modified (`bundled.rs`). Production
  code ~80 lines (struct + const + two public wrappers +
  two private `_from_exe_parent` seams; plus a small
  refactor of the existing `resolve_plugin_dir` to a
  matching seam for test symmetry). ~9 unit tests inside
  the same file's `#[cfg(test)]` block. Code + tests
  ship together per CLAUDE.md "Tests and Business Logic:
  Same Commit, Always" — this is the cohesive
  `bundled`-module API surface, not a split candidate.

### c02 — fix(rafaello): `rfl init` swaps shim for runtime binary at materialisation time

- **What.** Scope §A2 + the test-plumbing follow-on from
  pi-2 B-1. Edit
  `rafaello/crates/rafaello/src/init.rs::run`:
  1. Replace `const BUNDLED_OPENAI: &str = "openai"` with
     the imported `OPENAI_NAMES` constant from `bundled`.
  2. Change `let source_dir = bundled::resolve_plugin_dir(
     BUNDLED_OPENAI)?;` (line 77) to `let source_dir =
     bundled::resolve_plugin_dir_for_bundled(&OPENAI_NAMES)?;`.
  3. After `pp1::materialise(...)` returns `target_dir`
     (line 109), add:
     ```rust
     let runtime = bundled::resolve_runtime_binary(&OPENAI_NAMES)?;
     let entry_absolute = target_dir.join(manifest.entry.as_str());
     std::fs::copy(&runtime, &entry_absolute).map_err(/* InitError::Io */)?;
     #[cfg(unix)]
     {
         use std::os::unix::fs::PermissionsExt;
         std::fs::set_permissions(
             &entry_absolute,
             std::fs::Permissions::from_mode(0o755),
         ).map_err(/* InitError::Io */)?;
     }
     ```
  4. Move the `content_digest(&target_dir)?` call (line
     111) to *after* the copy + chmod, so the lock's
     `digest` field reflects the post-swap bytes.
  5. On `resolve_runtime_binary` `NotFound`, the existing
     `InitError::Bundled(BundledError)` propagation
     suffices. The materialised `target_dir` is removed on
     this failure: add a `std::fs::remove_dir_all(
     &target_dir).ok()` cleanup before returning the
     error, so a retry against a fixed environment starts
     clean (scope §A3).
  6. **(Round 3, pi-2 B-1 fold.)** Update each existing
     `rfl_init_*` test that reaches `pp1::materialise`
     (listed in the round-3 banner above) to add one
     `.env("RFL_BUNDLED_BIN_OPENAI",
     workspace_bin("rfl-openai-stub"))` line before
     `.output()` / `.spawn()`. Tests that short-circuit
     before materialise (decline, EOF, idempotent, --help)
     are not touched. Per-test diff is exactly one line.
- **Why.** Scope §A2/§A3. The defect: today's `init::run`
  copies the m4-c20 shim (`#!/bin/sh\nexec "$@"`) into
  `${PROJECT_ROOT}/.rafaello/plugins/<topic>/bin/rfl-openai`
  and seals the lock with the shim's content digest. The
  resulting `entry_absolute` is the shim, which when
  exec'd under syd falls back to `/bin/sh` (not in
  `exec_dirs` grant) → `Permission denied`. The fix
  materialises the real runtime binary in-place at the
  PP1-canonical path, preserving the PP1 containment
  invariant (`compile::resolve_entry`'s `EntryEscape`
  check stays trivially satisfied) and yielding a digest
  that matches the real binary.
- **Depends on.** c01 (needs `OPENAI_NAMES`,
  `resolve_plugin_dir_for_bundled`,
  `resolve_runtime_binary`).
- **Acceptance.** New test
  `rafaello/crates/rafaello/tests/rfl_init_materialises_real_runtime_binary.rs`
  (scope §C1):
  - Set `RFL_BUNDLED_PLUGINS_DIR=<tmp-fixture>` where
    `<tmp-fixture>/openai/` mirrors
    `rafaello/crates/rafaello-openai/` (manifest +
    openrpc + the in-tree shim — exact fixture vintage).
  - Set `RFL_BUNDLED_BIN_OPENAI=<workspace_bin("rfl-openai-stub")>`
    (real workspace-built binary used as the runtime
    sentinel; ~29 MB in debug per pi-1 confirmation).
  - Call `rafaello::init::run(InitArgs { yes: true, force:
    true, project_root: Some(tmp.path().to_path_buf()) })`.
  - Assert no error.
  - Read both files into `Vec<u8>` and assert
    `std::fs::read(&materialised)? == std::fs::read(&stub)?`
    (raw byte equality — no new `sha2`/`data-encoding` dep
    needed per pi-1 M-2). The equivalent coverage is
    "byte-identical to the stub binary".
  - Assert the file size is > 1024 bytes and that its
    first bytes are not the literal shim shebang
    `#!/bin/sh\nexec` (defensive — protects against a
    future regression where the copy step is skipped).
  - Parse the lock TOML, extract
    `[plugins.<canonical>].digest`, and assert it equals
    a fresh `rafaello_core::digest::content_digest(&target_dir)`
    computed against the post-swap install dir (scope §A2
    step 5).
  - Negative variant in the same file (or a sibling
    test): set `RFL_BUNDLED_BIN_OPENAI` to a non-existent
    path; assert `init::run` returns
    `InitError::Bundled(BundledError::NotFound { .. })`
    AND the temp `tmp/.rafaello/plugins/<topic>/`
    directory was cleaned up (does not exist after the
    error) AND the lock was not written.
- **Size.** 9 files: `init.rs` (~30 line delta), 1 new
  test file (~120 lines, C1), 7 existing
  `rfl_init_*.rs` tests (1 line each, totalling ~7
  lines). Cumulative diff ~160 lines. Above the
  CLAUDE.md ≤5-file guideline; defended as one cohesive
  unit per "Tests and Business Logic: Same Commit,
  Always" — the production change requires the test
  plumbing update for the suite to stay green. Pi may
  push back to split into c02a/c02b (see round-3 banner);
  driver's lean is one commit. **Per-commit agent must
  take care** not to drift into c01's helper definitions
  — the helpers are imported, not redefined.
- **Acceptance command.** `cargo test --manifest-path
  rafaello/Cargo.toml --workspace --features
  rafaello-core/test-fixture -p rafaello --test
  rfl_init_materialises_real_runtime_binary`.

### c03 — test(rafaello): `rfl init` subprocess regression — no `CARGO_BIN_EXE_*` env leak

- **What.** Scope §C2. New test
  `rafaello/crates/rafaello/tests/rfl_init_runtime_binary_outside_cargo_env.rs`:
  1. Skip-guard at top of `#[test]` fn: if
     `std::env::var_os("CARGO_TARGET_DIR")
     .map(|v| !v.is_empty()).unwrap_or(false)`, print a
     diagnostic and return. (Conservative skip per scope
     §C2 / pi-2 M-1 / pi-3 N-2.)
  2. Build `rfl = workspace_bin("rfl")` and
     `rfl_openai = workspace_bin("rfl-openai")` (the real
     workspace-target binary, not the stub).
  3. Build a temp source-tree fixture mirroring
     `rafaello/crates/rafaello-openai/`: copy
     `rafaello.toml`, `openrpc.json`, and the shim under
     `bin/rfl-openai` into a temp dir at `<tmp>/openai/`.
  4. Create a temp project root.
  5. Spawn `rfl init` as a subprocess:
     ```rust
     let status = std::process::Command::new(&rfl_path)
         .arg("init")
         .arg("--yes")
         .arg("--project-root").arg(project_root.path())
         .env_clear()
         .env("PATH", std::env::var_os("PATH").unwrap_or_default())
         .env("HOME", std::env::var_os("HOME").unwrap_or_default())
         .env("RFL_BUNDLED_PLUGINS_DIR", &fixture_root)
         .status()
         .expect("spawn rfl init");
     ```
     Note: **`RFL_BUNDLED_BIN_OPENAI` is deliberately NOT
     set**. The test exercises the dev-fallback arm
     (walk up from `<rfl-exe-parent>` = `<workspace>/target/<profile>/`
     to find `<workspace>/target/<profile>/rfl-openai`).
  6. Assert `status.success()`.
  7. Locate `<project-root>/.rafaello/plugins/<topic>/bin/rfl-openai`
     where `<topic> = rafaello_core::topic_id::derive(
     "builtin:openai@0.0.0")` (the canonical id is fixed
     by init; `topic_id::derive` is public per
     `rafaello-core/src/topic_id.rs`). Direct derive
     avoids coupling the path lookup to the lock-parse
     step that the same test later verifies (pi-1 N-1).
  8. Assert the materialised file size > 1024 bytes
     (heuristic per scope §C2; the cargo-built
     `rfl-openai` in debug is hundreds of KB, in release
     ~MB — both >> 1024).
  9. Parse `<project-root>/rafaello.lock`, extract
     `[plugins.<canonical>].digest`, recompute
     `content_digest(&target_dir)` against the post-swap
     install dir, assert equality (proves the lock
     digest tracks the real binary).
- **Why.** Scope §C2. C1 (c02's acceptance) exercises the
  explicit-override path; only C2 reproduces the
  owner-hit `cargo run --bin rfl -- init` cold-start
  layout via the subprocess + `.env_clear()` path. Without
  `.env_clear()`, `CARGO_BIN_EXE_*` from cargo test leaks
  through and the bug self-heals. This test is the
  regression anchor for the closed gap that masked D1
  through m6 ratification.
- **Depends on.** c02 (the init swap must exist for the
  test's assertions to hold) and c01 (the helpers).
- **Acceptance.** The test passes against the post-c02
  HEAD. Run via `cargo test --manifest-path
  rafaello/Cargo.toml --workspace --features
  rafaello-core/test-fixture -p rafaello --test
  rfl_init_runtime_binary_outside_cargo_env` (matches the
  scope demo-bar shape per pi-1 M-4). No code edits beyond
  the new test file.
- **Size.** 1 new file ~150 lines. Within budget.

### c04 — fix(rafaello-tui): Ctrl-C key event quits the TUI cleanly

- **What.** Scope §B1, §B4. Edit
  `rafaello/crates/rafaello-tui/src/bin/rfl_tui.rs`'s
  `handle_terminal_event` (lines 404–442 today):
  - Update the top-of-file `crossterm::event` import
    (lines 12–14) to add `KeyModifiers` to the existing
    import list.
  - Immediately after the `KeyEventKind::Release` early
    return (line 414), and **before** the mode dispatch
    at line 417's `match key.code`, add:
    ```rust
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('c')
    {
        return EventOutcome::Quit;
    }
    ```
    (No inline `use`; the top-level import covers it.)
- **Why.** Scope §B1, §B4. With `enable_raw_mode()` in
  effect (line 326), the kernel TTY discipline does not
  generate SIGINT on Ctrl-C; the byte arrives as a
  `KeyEvent` with `CONTROL+Char('c')`. The existing
  handler does not match it (only `Char('q')` quits in
  Normal mode), so the keystroke is silently swallowed.
  Wiring it to `EventOutcome::Quit` ahead of mode
  dispatch ensures both `InputMode::Normal` and
  `InputMode::ConfirmOverlay { .. }` honour the gesture.
  The existing Quit path runs `restore_terminal()` and
  returns Ok from `ui_loop`, which causes the child to
  exit; the parent's `handle.wait()` resolves, and the
  shutdown sequence (`lib.rs:578-585`) tears down cleanly.
  No `confirm_reply` is synthesised; m6.1 does not invent
  audit-deny semantics for in-flight confirmations (scope
  §B4, resolved per pi-1 M-2).
- **Depends on.** baseline (no c01–c03 dependency — D1
  and D2 are independent fixes; ordering is by scope-doc
  flow, not technical dependency).
- **Acceptance.** Extend the existing test module in
  `rfl_tui.rs` (around line 595) with a table-driven
  Ctrl-C test (scope §C3). Construct
  `KeyEvent { code: KeyCode::Char('c'), modifiers:
  KeyModifiers::CONTROL, kind: KeyEventKind::Press, state:
  KeyEventState::NONE }`. For each `mode` in
  `[InputMode::Normal, InputMode::ConfirmOverlay { /*
  minimal fixture */ }]`, call
  `handle_terminal_event(Event::Key(key), &mut mode, &mut
  scroll, &mut input_buffer, 0)` and assert the result
  matches `EventOutcome::Quit`. Negative variant: the
  same key without `CONTROL` (`KeyModifiers::NONE`) does
  **not** quit (proves the modifier check is required).
- **Size.** 1 file modified, ~10 lines production + ~40
  lines test. Within budget.
- **Acceptance command.** `cargo test --manifest-path
  rafaello/Cargo.toml --workspace --features
  rafaello-core/test-fixture -p rafaello-tui --bin rfl-tui
  -- handle_terminal_event` (the tests live in the
  `rfl_tui.rs` bin's `#[cfg(test)]` block, so the bin's
  test target is the surface).

### c05 — test(rafaello): tmux-driven Ctrl-C regression for `rfl chat`

- **What.** Scope §C4. New test
  `rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`.
  Mirrors `rfl_chat_production_tui_input_overlay_e2e.rs`
  (cK5) lines 47–275 closely. Concrete shape:
  0. File-top cfg gate
     `#![cfg(target_os = "linux")]` per pi-1 B-2. The
     lockin/syd/PTY stack the test exercises is
     Linux-only; cK5 carries the same gate. macOS / other
     hosts get a clean "no tests" rather than a
     spawn-time failure.
  0b. `rafaello/crates/rafaello/Cargo.toml`
     `[dev-dependencies]` gains
     `ulid = { workspace = true }` (per pi-1 B-1; the
     workspace already declares `ulid` but the `rafaello`
     crate did not depend on it directly).
  1. Skip if `tmux -V` fails (matches cK5 lines 47–48).
  2. `let tmp = tempfile::tempdir().unwrap();` — project
     root.
  3. `install_demo_layout(tmp.path(), InstallOptions {
     provider_executable: true, tool_executable: true,
     real_binaries: true });` (helper at
     `tests/common/m4_install.rs:36`).
  4. `let rfl = workspace_bin("rfl");`
     `let tui = workspace_bin("rfl-tui");`
  5. Write wrapper script at `tmp.path().join("rfl-chat-wrapper.sh")`:
     ```sh
     #!/bin/sh
     export RFL_TUI_PATH='<tui>'
     export TERM='xterm-256color'
     exec '<rfl>' chat --project-root '<project-root>' 2>'<stderr-log>'
     ```
     chmod `0o755`. `<stderr-log>` is `tmp.path().join("rfl-chat.stderr")`.
  6. Session name: `format!("rfl-c05-ctrlc-{}",
     ulid::Ulid::new())`. `ulid` crate is already in the
     workspace (used by `rfl-tui` per its imports).
  7. Install `TmuxSessionGuard { session: name.clone() }`
     immediately — Drop impl runs `tmux kill-session -t
     <session>` on drop.
  8. `tmux new-session -d -s <session> -x 100 -y 30
     <wrapper-script>` — wrapper passed as the pane
     command, not keyed in. Assert exit status success.
  9. Helper `poll_for_stderr_line(&log_path,
     "frontend-ready-observed", Duration::from_secs(30))`
     — copied verbatim from cK5's helper (or factored
     into a shared `tests/common/tmux_helpers.rs` if a
     follow-up cleanup is appropriate; round-1 lean is
     "copy into the test, do not refactor cK5"). Block
     until the parent-emitted sentinel
     `rfl-chat: frontend-ready-observed` appears in the
     stderr log.
  10. `tmux send-keys -t <session> C-c`.
  11. Poll `session_alive(&session)` (helper that shells
      out to `tmux has-session -t <session>` and returns
      `status.success()`) every 100ms for up to 5s; the
      loop exits as soon as `session_alive` returns
      false. Same shape as cK5 lines 228–244.
  12. Assert the session exited within the timeout. On
      timeout, capture-pane + read stderr log, include
      both in the panic message for debuggability.
  13. Read the captured stderr log; assert it does not
      contain `panicked` (defensive check for shutdown
      panics in the parent rafaello process).
- **Why.** Scope §C4. The C3 unit test proves the key
  handler converts `C-c` to `Quit`; only C4 proves the
  raw-mode TTY path delivers the byte all the way through
  crossterm's event stream and the parent/child stack
  tears down cleanly. This is the load-bearing regression
  anchor for D2.
- **Depends on.** c04 (the handler fix must exist;
  pre-c04 the test fails by `session_alive` timeout).
- **Acceptance.** The test passes against the post-c04
  HEAD. Run via `cargo test --manifest-path
  rafaello/Cargo.toml --workspace --features
  rafaello-core/test-fixture -p rafaello --test
  rfl_chat_ctrl_c_quits_cleanly` (matches scope demo-bar
  shape per pi-1 M-4). Test takes < 35s wall clock (30s
  ready timeout + 5s exit timeout + ~few s spawn).
- **Size.** 2 files modified: 1 new test file ~180 lines
  (cK5 is ~280 lines; C4 is a smaller surface — no
  overlay-key sequence, no SQLite assertions, no
  install) + a 1-line addition to
  `rafaello/crates/rafaello/Cargo.toml`'s
  `[dev-dependencies]` (ulid). Within budget.

### c06 — docs(rafaello-v0_1_1): `manual-validation.md` v0.1.1 appendix + transcripts

- **What.** Scope §D. Create
  `rafaello/plans/milestones/m6.1-cold-start-patches/manual-validation.md`
  with an appendix structure:
  - **Provenance header** (`transcripts/v0_1_1/00-CONTEXT.md`,
    new in round 2 per pi-1 M-3): captures the exact
    conditions of the recording so the transcripts are
    auditable evidence rather than illustrative samples.
    Required fields:
    - Date (ISO 8601).
    - Hostname or worktree path.
    - `git rev-parse HEAD` of the m6.1 branch at capture
      time (must be the tip of `agents/v0.1.1/driver`
      after c05 lands).
    - `tmux -V` output.
    - Terminal size used for capture (cols × rows).
    - `which rfl` and the `cargo build` invocation used
      to produce it.
    - Whether `CARGO_BIN_EXE_*` and
      `RFL_BUNDLED_BIN_OPENAI` were present in the
      capturing shell's env (expected: both absent for
      the cold-start authenticity).
    - For the `rfl chat` capture, whether
      `LITELLM_API_KEY` was set (the demo runs against
      the dev LiteLLM proxy when set; against
      the in-tree mock fixtures otherwise — call it out
      so the assistant-output lines can be interpreted).
  - **§1 cold-start.** Three captures under
    `transcripts/v0_1_1/`:
    - `01-cold-init.txt` — tmux capture of `rfl init
      --yes` against an empty project root from a
      freshly-built `rfl` (no `CARGO_BIN_EXE_*` env
      ambient). Asserted via visual inspection: lock
      written, no error, no interactive accept prompt
      rendered (`--yes` skips the prompt).
    - `02-cold-chat.txt` — tmux capture of `rfl chat`
      against the same project, showing the initial TUI
      render and the provider-spawn sentinel in stderr.
      Pre-fix would show `syd: exec error: Permission
      denied`; post-fix is clean.
    - `03-ctrl-c-exit.txt` — tmux capture spanning the
      Ctrl-C keystroke through the parent's exit.
      Pre-fix: the TUI stays alive. Post-fix: clean exit
      with terminal restored to the parent shell.
  - **Recording method.** A short shell-script-style
    recipe at the top of `manual-validation.md`
    documenting the tmux commands used to capture each
    transcript (so the appendix is reproducible on a
    different host). Pattern follows m6 cK6's
    `manual-validation.md` §5.1 appendix.
  - The three transcript files land under
    `transcripts/v0_1_1/` with literal `.txt` filenames
    and no terminal escape codes (tmux capture-pane
    output is plain text by default). The
    `00-CONTEXT.md` is markdown so the provenance
    metadata renders on GitHub.
- **Why.** Scope §D. Pre/post-fix evidence on disk so the
  retrospective + future operators can verify the cold-
  start UX is repaired against a real `rfl` binary.
  Mirrors m6 cK6's appendix pattern.
- **Depends on.** c02, c04 (the fixes must be live for
  the post-fix captures to be authentic).
- **Acceptance.** The three transcript files exist under
  `rafaello/plans/milestones/m6.1-cold-start-patches/transcripts/v0_1_1/`
  with substantive content. The appendix
  `manual-validation.md` references each by relative
  path. No automated test asserts the transcript
  contents; the appendix is documentation-grade
  evidence.
- **Size.** 5 new files (the appendix +
  `00-CONTEXT.md` + 3 transcripts). Within budget; all
  docs / capture content.

## Acceptance traceability appendix

| Scope section                          | Implemented by | Test/artefact                                              |
|----------------------------------------|----------------|------------------------------------------------------------|
| §A0 — `BundledPluginNames` + sister fn | c01            | `resolve_plugin_dir_for_bundled_*` unit tests              |
| §A0 — `resolve_plugin_dir` regression  | c01            | `resolve_plugin_dir_release_arm_uses_input_name_verbatim`  |
| §A1 — `resolve_runtime_binary`         | c01            | `resolve_runtime_binary_{env,release,dev,not_found,munge}` |
| §A2 — `init::run` swap + chmod         | c02            | `rfl_init_materialises_real_runtime_binary.rs`             |
| §A2 step 5 — digest post-swap          | c02            | (same test, digest equality assertion)                     |
| §A3 — cleanup on `NotFound`            | c02            | Negative variant in same test                              |
| §B1 — Ctrl-C handler                   | c04            | Unit test in `rfl_tui.rs` test module                      |
| §B4 — no `confirm_reply` synthesis     | c04 (passive)  | (Negative: nothing extra is wired — `Quit` path unchanged) |
| §C1 — in-process acceptance            | c02            | `rfl_init_materialises_real_runtime_binary.rs`             |
| §C2 — subprocess no-override regression| c03            | `rfl_init_runtime_binary_outside_cargo_env.rs`             |
| §C3 — Ctrl-C unit test                 | c04            | Same test module additions                                 |
| §C4 — tmux Ctrl-C regression           | c05            | `rfl_chat_ctrl_c_quits_cleanly.rs`                         |
| §D — manual-validation appendix        | c06            | `manual-validation.md` + `00-CONTEXT.md` + 3 transcripts   |

## Cross-checks

- **Existing `c21` tests still pass after c02.** c02
  swaps the shim for the real binary post-`pp1::materialise`.
  The new resolver does **not** consult
  `CARGO_BIN_EXE_*` (deliberate per scope §A1 and pi-2
  B-1's correction). Therefore c02 also adds one
  `.env("RFL_BUNDLED_BIN_OPENAI",
  workspace_bin("rfl-openai-stub"))` line to each existing
  `rfl_init_*` test that reaches `pp1::materialise`
  (listed in the round-3 banner). With that env set, each
  test's `init::run` invocation finds the stub binary,
  swaps it into the install dir, and the test's existing
  assertions hold (file count unchanged,
  lock shape unchanged, digest still computes from
  `target_dir` — now reflecting the stub bytes, not the
  shim bytes; the existing tests do not assert digest
  values, only digest presence / round-trip).
  Tests that short-circuit before `pp1::materialise`
  (decline, EOF, idempotent, --help) are unaffected by
  the resolver change and are not modified.
- **`rfl install rfl-mailcat` (m6 c-Phase-B) unchanged.**
  c01 explicitly does not modify
  `bundled::resolve_plugin_dir(name)`; `install.rs:96`
  continues to call it with the operator's positional
  string. The `resolve_plugin_dir_release_arm_uses_input_name_verbatim`
  unit test is the regression guard.
- **m6 cK5 test still passes.** c04's `rfl-tui` edits
  add a Ctrl-C branch but do not remove any existing
  handler. cK5's existing input + overlay-key
  assertions remain untouched.
- **No `overview.md` / `decisions.md` / `glossary.md`
  edits in c01–c06.** Retrospective phase appends one
  `decisions.md` row (the materialisation-time
  runtime-binary swap contract) and optionally one
  `glossary.md` entry. No `overview.md` structural
  drift.

## Sizing summary

| Commit | Files | Approx LoC | Test coverage              |
|--------|-------|------------|----------------------------|
| c01    | 1     | ~150       | 9 unit tests in-file       |
| c02    | 9     | ~160       | 1 new C1 + 7 existing tests updated 1 line each |
| c03    | 1     | ~150       | 1 integration test (C2)    |
| c04    | 1     | ~50        | unit test in-file (C3)     |
| c05    | 2     | ~180 + 1   | this **is** the test (C4)  |
| c06    | 5     | (docs)     | n/a — manual evidence      |

All commits respect the CLAUDE.md ≤5-file / ≤100-line
production-code envelope. c01 and c05 hit ~150–180 LoC
because the test count is intentionally thorough — pi
may push back if any line counts feel padded.

## References

- `scope.md` round 5 CONVERGED at `6912344`.
- m6 cK5 precedent:
  `rafaello/crates/rafaello/tests/rfl_chat_production_tui_input_overlay_e2e.rs`.
- m4 c20 shim:
  `rafaello/crates/rafaello-openai/bin/rfl-openai`.
- m6 c21 init impl:
  `rafaello/crates/rafaello/src/{init.rs,bundled.rs,pp1.rs}`.
- PP1 release layout: `rafaello/nix/package.nix:37-63`.

## Disagreements with pi (cumulative)

None across rounds 1–2. All three blockers (pi-1 B-1/B-2,
pi-2 B-1), all four majors, and all five nits were
substantive and accurate; every one is folded.
