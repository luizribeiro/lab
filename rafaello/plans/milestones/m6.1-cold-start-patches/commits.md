# m6.1 — v0.1.1 cold-start patches — commits

> **Status:** round 1 — claude-authored 2026-05-12, awaiting
> pi round 1. Built on top of `scope.md` round-5 CONVERGED
> (`6912344`, owner-ratified). Six commits, sized 5–10 per
> the owner directive.

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
  module:
  1. `resolve_plugin_dir_release_arm_uses_input_name_verbatim`
     — pre-existing helper `resolve_plugin_dir("rfl-mailcat")`,
     `RFL_BUNDLED_PLUGINS_DIR` unset, set up a temp
     hierarchy with `<exe-parent>/../share/rafaello/plugins/rfl-mailcat/`
     present and `<exe-parent>/../share/rafaello/plugins/rfl-rfl-mailcat/`
     absent. Assert the function returns the former (regression
     guard — proves m6.1 did not break `rfl install`).
  2. `resolve_plugin_dir_for_bundled_env_arm_hit` — set
     `RFL_BUNDLED_PLUGINS_DIR=<tmp>` and create
     `<tmp>/openai/rafaello.toml`. Assert
     `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)` returns
     `<tmp>/openai`.
  3. `resolve_plugin_dir_for_bundled_release_arm_hit` —
     `RFL_BUNDLED_PLUGINS_DIR` unset, set up a temp tree
     with `<exe-parent>/../share/rafaello/plugins/rfl-openai/`
     present. Assert the function returns that directory.
  4. `resolve_plugin_dir_for_bundled_dev_fallback_hit` —
     no env, no release tree; assert the function finds the
     in-tree `crates/rafaello-openai/` (uses
     `env!("CARGO_MANIFEST_DIR")` to construct expected
     path; the dev-fallback walks up from the test binary
     and lands on the workspace).
  5. `resolve_runtime_binary_env_override_hit` —
     `RFL_BUNDLED_BIN_OPENAI=<tmp>/sentinel` where
     `<tmp>/sentinel` is a 0o755 regular file; assert the
     resolver returns that path. Negative variant: same
     path but mode 0o644 → `NotFound`.
  6. `resolve_runtime_binary_release_arm_hit` — set up a
     temp `<exe-parent>/../share/rafaello/plugins/rfl-openai/bin/rfl-openai`
     0o755 file; no envs set; assert the resolver returns
     that path.
  7. `resolve_runtime_binary_dev_fallback_hit` — no env,
     no release tree; touch
     `<workspace>/target/debug/rfl-openai` 0o755 (or rely
     on a cargo-built one); assert the resolver returns
     that path.
  8. `resolve_runtime_binary_not_found_lists_all_arms` —
     no env, no release tree, no workspace; assert
     `BundledError::NotFound`. Inspect the `Display`
     impl's message and assert it contains the literal
     `RFL_BUNDLED_BIN_OPENAI`, the release-arm path
     substring, and the dev-fallback substring.
  9. `resolve_runtime_binary_env_var_name_munge` — call
     `resolve_runtime_binary` with a `BundledPluginNames
     { dev_crate: "foo-bar", .. }`, ensure the env-var
     lookup is `RFL_BUNDLED_BIN_FOO_BAR` (not
     `RFL_BUNDLED_BIN_foo-bar`) — assertion via setting
     the expected env var and observing the override hit.
  - All tests use `tempfile::tempdir` and `serial_test`
    (already a workspace dep) or `std::env::remove_var`
    /`std::env::set_var` inside a guard struct, to avoid
    cross-test env pollution.
- **Size.** ~3 files touched (`bundled.rs` only, plus its
  test module if split). ~150 lines added (struct, two
  sister fns, ~9 test cases). Within the 100-line / 3–5
  file budget when measured on the production code alone;
  the test cases are below the per-test review threshold.
  Pi may flag if the test count is bundled too loosely —
  acceptable to split tests into a second commit if
  needed, but round-1 lean is "single c01 lands code +
  its own tests" per the workspace's CLAUDE.md commit
  guidelines ("Tests and Business Logic: Same Commit,
  Always").

### c02 — fix(rafaello): `rfl init` swaps shim for runtime binary at materialisation time

- **What.** Scope §A2. Edit
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
  - Compute SHA-256 of the materialised file at
    `tmp/.rafaello/plugins/<topic>/bin/rfl-openai` and of
    the `rfl-openai-stub` binary; assert equal.
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
- **Size.** 2 files modified (`init.rs` ~30 line delta +
  one new test file ~120 lines). Within budget. Note
  per-commit agent must take care not to drift into c01's
  helper definitions — the helpers are imported, not
  redefined.

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
     (compute `<topic>` via `topic_id::derive("builtin:openai@0.0.0")`
     — or read the lock and extract the canonical id /
     topic from there).
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
  rafaello/Cargo.toml -p rafaello --test
  rfl_init_runtime_binary_outside_cargo_env`. No code
  edits beyond the new test file.
- **Size.** 1 new file ~150 lines. Within budget.

### c04 — fix(rafaello-tui): Ctrl-C key event quits the TUI cleanly

- **What.** Scope §B1, §B4. Edit
  `rafaello/crates/rafaello-tui/src/bin/rfl_tui.rs`'s
  `handle_terminal_event` (lines 404–442 today):
  - Immediately after the `KeyEventKind::Release` early
    return (line 414), and **before** the mode dispatch
    at line 417's `match key.code`, add:
    ```rust
    use crossterm::event::KeyModifiers;
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('c')
    {
        return EventOutcome::Quit;
    }
    ```
  - Update the top-of-file `crossterm::event` import to
    add `KeyModifiers` to the existing list (line 12–14).
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

### c05 — test(rafaello): tmux-driven Ctrl-C regression for `rfl chat`

- **What.** Scope §C4. New test
  `rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`.
  Mirrors `rfl_chat_production_tui_input_overlay_e2e.rs`
  (cK5) lines 47–275 closely. Concrete shape:
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
  rafaello/Cargo.toml -p rafaello --test
  rfl_chat_ctrl_c_quits_cleanly`. Test takes < 35s wall
  clock (30s ready timeout + 5s exit timeout + ~few s
  spawn).
- **Size.** 1 new file ~180 lines (cK5 is ~280 lines; C4
  is a smaller surface — no overlay-key sequence, no
  SQLite assertions, no install). Within budget.

### c06 — docs(rafaello-v0_1_1): `manual-validation.md` v0.1.1 appendix + transcripts

- **What.** Scope §D. Create
  `rafaello/plans/milestones/m6.1-cold-start-patches/manual-validation.md`
  with an appendix structure:
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
    output is plain text by default).
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
- **Size.** 4 new files (the appendix + 3 transcripts).
  Within budget.

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
| §D — manual-validation appendix        | c06            | `manual-validation.md` + 3 transcripts                     |

## Cross-checks

- **Existing `c21` tests still pass.** c02 swaps the shim
  for the real binary post-`pp1::materialise`, but the
  c21 tests (`rfl_init_writes_default_lock.rs`,
  `rfl_init_materialises_package_dir.rs`,
  `rfl_init_force_rewrites.rs`,
  `rfl_init_idempotent_no_overwrite.rs`) all run under
  cargo test with `RFL_BUNDLED_PLUGINS_DIR` set to an
  in-tree fixture and `CARGO_BIN_EXE_*` available. c02's
  edits do not break any of those assertions —
  `pp1::materialise`'s file count is unchanged, the lock
  shape is unchanged, the digest field still computes
  from `target_dir`. The c21 tests' file-presence checks
  remain green because the swap overwrites in place
  rather than adding/removing files.
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
| c02    | 2     | ~150       | 1 integration test (C1)    |
| c03    | 1     | ~150       | 1 integration test (C2)    |
| c04    | 1     | ~50        | unit test in-file (C3)     |
| c05    | 1     | ~180       | this **is** the test (C4)  |
| c06    | 4     | (docs)     | n/a — manual evidence      |

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

(None yet — pi review round 1 pending.)
