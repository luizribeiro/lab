# m6.1 scope.md — pi review round 1

Verdict: BLOCKING

Counts: B/2 M/5 N/4

## Blockers

### B-1: §A1's release-runtime lookup contradicts the ratified PP1 release layout

Scope §A1 lines 100-103 says the release runtime binary is at `<rfl-exe-parent>/rfl-<name>` and claims this matches the m6 PP1 contract where top-level `bin/` carries each plugin runtime binary. That is not the ratified/live m6 layout:

- `decisions.md` row 59/65: top-level `$out/bin/` retains only `rfl` and `rfl-tui`; plugin binaries are real files under `$out/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`.
- `rafaello/nix/package.nix:37-63` implements that: it copies `$out/bin/$name` into `$out/share/rafaello/plugins/$name/bin/$name` and then `rm "$bin_src"`.

So a released v0.1.1 `rfl init` would take the §A2 post-materialisation swap path, call `resolve_runtime_binary("openai")`, fail to find top-level `bin/rfl-openai`, and exit non-zero even though the release tree already has the real plugin binary in the PP1 plugin tree. This is material: the scope fixes the checkout/dev repro while making the installed-release path depend on a binary location that m6 explicitly removed.

Concrete fix proposal:

1. Change the release arm for `resolve_runtime_binary("openai")` to try the PP1 plugin-tree location, e.g. `<rfl-exe-parent>/../share/rafaello/plugins/rfl-openai/bin/rfl-openai` (or a clearly specified `runtime_plugin_dir(name)` mapping from provider id `openai` to release plugin dir `rfl-openai`).
2. Also resolve the source-tree release naming mismatch explicitly. Current `bundled::resolve_plugin_dir("openai")` looks for `<bin>/../share/rafaello/plugins/openai` (`bundled.rs:27-36`), while `package.nix` writes `rfl-openai`. Either m6.1 must include a small resolver compatibility fix (`openai` -> `rfl-openai` for release layout) or the scope must state why release `rfl init` is not part of v0.1.1. I do not think the latter is acceptable for a patch to the just-shipped cold-start flow.
3. Update the A1 error text and C2/C1 fixtures to cover both dev-source `openai/` and release-tree `rfl-openai/` naming, so this does not remain hidden by test-only `RFL_BUNDLED_PLUGINS_DIR` layouts.

### B-2: §C2's “cold-start regression” still uses the new runtime override, so it does not test the owner-hit no-override path

Scope §C2 lines 253-261 says the subprocess regression test runs `rfl init` under `.env_clear()` but then allowlists both `RFL_BUNDLED_PLUGINS_DIR` and `RFL_BUNDLED_BIN_OPENAI`. That proves the explicit override works; it does not prove the default runtime-binary discovery that a user gets from `cargo run --bin rfl -- init` or an installed `rfl` with no `RFL_BUNDLED_BIN_OPENAI`.

This matters because D1's repro in §Goal lines 21-26 is the no-override cold-start path from a fresh checkout. With C2 as written, these regressions would still pass:

- the dev fallback in §A1 lines 104-111 is broken;
- the release layout in §A1 lines 100-103 points at the wrong location (B-1);
- the resolver accidentally becomes “tests only” because `RFL_BUNDLED_BIN_OPENAI` is always required in subprocess coverage.

C1 is the right place to assert exact byte equality via an explicit sentinel override. C2 should be the no-runtime-override subprocess regression.

Concrete fix proposal:

- In C2, set `RFL_BUNDLED_PLUGINS_DIR` to the shim-containing fixture tree, but do **not** set `RFL_BUNDLED_BIN_OPENAI`.
- Keep `.env_clear()` and explicitly preserve only `PATH`, `HOME`, `RFL_BUNDLED_PLUGINS_DIR`, and any env genuinely needed by the platform. Do not preserve `CARGO_BIN_EXE_*`.
- Let `resolve_runtime_binary("openai")` find the workspace `target/<profile>/rfl-openai` via its dev fallback. That is the subprocess version of the owner-hit checkout path.
- Keep the explicit-env override coverage in c01 unit tests / C1, where it belongs.

## Majors

### M-1: §A1 dev fallback is not actually “same shape as workspace_bin_path” when `CARGO_TARGET_DIR` is external

Scope §A1 lines 104-111 says the dev fallback mirrors `tests/common/workspace_bin_path.rs`. It only mirrors the default-target case. The helper finds the workspace from compile-time `CARGO_MANIFEST_DIR` and honors `CARGO_TARGET_DIR` (`workspace_bin_path.rs:4-31`). The proposed prod resolver walks upward from `current_exe()`'s parent. That works for the common `cargo run --bin rfl` default target (`rafaello/target/debug/rfl` -> walk up to `rafaello/Cargo.toml` with `[workspace]`), but fails when Cargo is invoked with an external `CARGO_TARGET_DIR` (`/tmp/target/debug/rfl` has no workspace ancestor).

This does not need to block the owner-hit default repro, but the scope should not claim exact parity with `workspace_bin_path`. Either:

- add a second dev fallback that walks up from `std::env::current_dir()` looking for `[workspace]`, then uses that workspace root's `target/<profile>/rfl-<name>`; or
- explicitly state the dev fallback covers default-target `cargo run` only, with `RFL_BUNDLED_BIN_OPENAI` as the escape hatch for external target dirs.

If B-2 is fixed so C2 exercises no-override dev fallback, this needs to be nailed down to avoid CI/devenv surprises.

### M-2: §B4 invents a parent-side “implicit deny” path that does not exist

Scope §B4 lines 213-225 says Ctrl-C while a confirm modal is open “implicitly denies the in-flight confirmation” because “the confirmation reply task in the parent observes the child's exit and treats unanswered confirmations as session torn down.” I do not find that path in the code.

The live shutdown shape is:

- `rfl_tui.rs:404-441` handles key events locally; no confirm answer is published on Quit.
- `lib.rs:748-755` observes either OS SIGINT or `handle.wait()` for frontend exit.
- after `run_post_ready` returns, `lib.rs:578-585` sends the broad shutdown watch, waits router/agent/slash tasks, aborts the gate task, drops plugin handles, and shuts down the plugin supervisor.
- the gate itself only resolves confirmations on `core.session.confirm_reply` or timeout (`gate/mod.rs:1-30`, `:199-200`, `:532+`). There is no “child exited, synthesize deny” confirm-reply path.

The desired UX is still reasonable: Ctrl-C exits the TUI and tears down the session. But calling it an implicit deny is stronger than the implementation and could make c04/c05 agents add speculative parent-side behavior that §B3 says is out of scope.

Concrete fix proposal: reword §B4 to: “Ctrl-C while a confirm modal is open exits the TUI; the parent observes frontend exit via `handle.wait()`, then runs the existing shutdown sequence. No `confirm_reply` is synthesized and no audit-log deny row is specified in m6.1.” Keep the C3 handler assertion for both modes.

### M-3: §C4's tmux child-PID/reap plan is underspecified and `Pid::wait` is the wrong primitive for a tmux-spawned process

Scope §C4 lines 290-300 says the test spawns `rfl chat` under tmux, polls the child PID, and performs a “reap check via `Pid::wait`.” For the cK5-style tmux harness, the `rfl` process is spawned by the tmux server/shell, not by the Rust test process. A Rust test cannot `waitpid` an arbitrary tmux pane process unless it is that process's parent; `nix::sys::wait::waitpid` would fail with `ECHILD`. There is also no standard `Pid::wait` API that makes this clear.

The m6 cK5 precedent (`rfl_chat_production_tui_input_overlay_e2e.rs`) avoids this: it uses a tmux session guard, polls the tmux session for liveness after sending `q`, and does not claim to reap a non-child. If m6.1 wants stronger process-status assertions, the scope must specify a valid ownership model.

Concrete fix proposal:

- If keeping tmux: have the wrapper `exec` `rfl` so `#{pane_pid}` is the `rfl` pid, poll tmux/session or `/proc/<pid>` for disappearance, and drop the “reap via waitpid” claim. You can assert the tmux session exits and no live descendant remains, but you cannot reap it from the test process.
- If actual reaping is required: spawn `rfl` as a child of the test process attached to a PTY you control, not under tmux; then use `tokio::process::Child::wait`/`std::process::Child::wait`.

### M-4: §C4 session naming needs to require a unique nonce, not leave PID as an acceptable example

Scope §C4 lines 290-292 says the tmux session name is derived from test name + nonce, and owner-judgment item 5 lines 411-415 gives “PID-suffix or `Ulid`-suffix” as examples. PID is not a sufficient nonce in Rust integration tests: cargo can run multiple tests in the same test binary/process, and even separate test binaries can collide with stale tmux sessions if the name is reused after a crash. The m1-m6 guardrail is “never reuse a name,” and the Phase K precedent used a nonce because stale sessions were a recurring operational hazard.

Concrete fix proposal: require `Ulid::new()` (or an equivalent random/monotonic nonce) in the session name and mark the test `#[serial(rfl_chat)]` if it touches shared process/env/syd/tmux resources. Drop PID as an acceptable example.

### M-5: §C4 overstates the unit-test weakness with a false pre-fix claim

Scope §C4 lines 304-306 says “a unit test of the key handler would have passed before the fix.” It would not: current `handle_terminal_event` first ignores releases, then matches `KeyCode::Char('q')`, arrows, and otherwise dispatches by mode (`rfl_tui.rs:411-441`). In Normal mode, `Ctrl-C` is still `KeyCode::Char('c')` and `handle_normal_key` ignores modifiers and appends `c`, returning `Redraw`; in ConfirmOverlay it routes to overlay-key handling, not `Quit`.

The correct point is that a unit test alone would not prove the raw-mode/TTY delivery path. Reword to: “The C3 unit test would fail pre-fix and pass post-fix, but only C4 proves the real tmux/raw-mode path delivers `C-c` to the handler and exits the parent/child stack cleanly.”

## Nits

### N-1: §A1 not-found message names the env var incorrectly

Scope §A1 lines 112-115 says the message tried `env <NAME>_BIN_OPENAI`. The env var specified in lines 93-99 is `RFL_BUNDLED_BIN_<NAME_UPPER>`, e.g. `RFL_BUNDLED_BIN_OPENAI`. Fix the literal to avoid confusing test assertions and operator diagnostics.

### N-2: §A2 step 4 contradicts §A1's executable validation

Scope §A1 lines 97-99 says the resolver validates the env override is executable or returns `NotFound`; §A2 lines 126-129 then says the resolver may return a binary with non-exec mode. The defensive chmod is fine, but the rationale should be “preserve/normalize mode after copy” rather than “resolver may return non-exec.”

### N-3: §D's `01-cold-init.txt` should state whether it is interactive or `--yes`

Scope §D lines 316-319 says the cold-init transcript shows the accept prompt. C2 and the m6 bootstrap use `--yes`, which skips the prompt. If the manual transcript is intentionally interactive, say “run without `--yes`, type `y`.” If it is the bootstrap command, say `--yes` and drop “accept prompt shown.”

### N-4: Acceptance summary “No other code touched” is too absolute

Scope lines 502-506 already require a `decisions.md` row and this milestone obviously touches tests/code for the two defects. Reword “No other code touched” to “No unrelated code touched” or “No code outside the named D1/D2 surfaces touched.”

## Items the scope handles correctly (confirmation)

- §A2 gets the digest ordering right: `init.rs` currently computes `content_digest(&target_dir)` immediately after `pp1::materialise` (`init.rs:104-112`), and `compile_plugin` rejects stale content digests before entry resolution (`compile.rs:155-163`). The swap must happen before the digest.
- §A4's reason for keeping the in-tree shim is real: `validate_with_package` resolves `manifest.entry` at validation time and requires a canonicalised regular file inside `package_dir` (`validate_with_package.rs:18-34`, `:91-114`). Removing the shim would break in-tree validation.
- §A5's “real bytes, not symlink” conclusion matches PP1: `compile::resolve_entry` canonicalises the entry and returns `EntryEscape` if it leaves the package dir (`compile.rs:440-466`).
- §B1 is the right fix layer for D2. Raw mode prevents kernel SIGINT generation; the child key handler must convert `Ctrl-C` into `EventOutcome::Quit`. The parent `tokio::signal::ctrl_c()` paths in `lib.rs:647-660` and `:748-755` are not the missing piece.
- §C2's `env_clear()` instinct is right. Reading `init::run` end-to-end shows `rfl init --yes --project-root <tmp>` needs filesystem access, `current_exe`, the bundled-source/runtime resolver, and lock writes; it does not need `TMPDIR`, `USER`, locale vars, or `RUST_BACKTRACE` for the asserted success path. Keep `PATH`/`HOME` if the harness/wrapper needs them.
- The C2 size heuristic is sound when paired with C1 exact-byte equality: I built `rfl-openai-stub` locally and `target/debug/rfl-openai-stub` is 29,593,944 bytes, comfortably above `>1024`.
- The owner-ratified choices I was told not to relitigate are respected in the draft: directory name `m6.1-cold-start-patches`, C2 stopping at lock-correctness, and a new explicit `RFL_BUNDLED_BIN_OPENAI` override.
- The `NAME_UPPER` hyphen-to-underscore rule is harmless future-proofing, not a ratification problem, as long as the only v0.1.1 exercised value remains `openai -> OPENAI`.
- Leaving audit-log assertions out of C4 is correct: m6 does not specify a graceful-shutdown audit row for Ctrl-C exit.
- Deferring Ctrl-D/Esc and parent-side signal-forwarding refactors is consistent with the two-defect patch scope.

## Out-of-scope checks performed (negative coverage)

- I checked the confirmation gate/re-emit path for a hidden “frontend exited means deny” behavior and did not find one; see M-2. This is a wording/scope-risk issue, not a request to add parent-side confirmation semantics in m6.1.
- I checked whether `manifest::validate_with_package` merely checks path syntax; it does more than that and requires real files, so A4 is not hand-wavy.
- I checked whether symlinking `entry_absolute` outside the package dir could be a shortcut; `compile::resolve_entry` rejects it, so A5 is correct.
- I checked whether `rfl init` appears to need extra env after `.env_clear()`; it does not for the planned non-interactive `--project-root` path.
- I checked the existing cK5 tmux pattern. It supports tmux-driven raw-mode testing, but it does not support `waitpid` reaping of tmux-spawned children; C4 needs the M-3 clarification.
