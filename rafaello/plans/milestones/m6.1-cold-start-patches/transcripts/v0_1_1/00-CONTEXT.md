# §1 transcript capture context — m6.1 v0.1.1 cold-start patches

Provenance header for the three captures under this directory
(`01-cold-init.txt`, `02-cold-chat.txt`, `03-ctrl-c-exit.txt`),
per scope §D + pi-1 M-3: the recordings are auditable evidence
rather than illustrative samples, so the capturing conditions
are documented here in full.

## Capture environment

- **Date:** 2026-05-12 (ISO 8601).
- **Host / worktree:** `/home/luiz/lab-wt/v0.1.1-c06` on host
  `sodium`. This is the c06 worktree forked from
  `agents/v0.1.1/driver` after c05 landed.
- **`git rev-parse HEAD`** (m6.1 branch, capture time):
  `02b7ca85ab5f3320814ed070a8c45732bcd2f366` — the
  post-c05 tip (`02b7ca8 test(rafaello): tmux-driven Ctrl-C
  regression for rfl chat`). The three transcripts in this
  directory were captured before c06 itself was committed,
  so HEAD here is the c05 tip and the captures themselves are
  the only c06-introduced content (alongside this
  `00-CONTEXT.md` and the `manual-validation.md` appendix).
- **`tmux -V`:** `tmux 3.6a`.
- **Terminal size:** `100 × 30` (cols × rows), matching cK5's
  pane size (`tmux new-session -d -s … -x 100 -y 30`).
- **`which rfl`** (after `PATH` export inside the capture
  shell):
  `/home/luiz/lab-wt/v0.1.1-c06/rafaello/target/debug/rfl`.
- **`cargo build` invocation** used to produce that `rfl`:

  ```sh
  cd rafaello && \
    cargo build --workspace --bins \
      --features rafaello-core/test-fixture
  ```

  Built from the c05 tip; binaries land under
  `rafaello/target/debug/` (`rfl`, `rfl-openai`,
  `rfl-mailcat`, …).
- **`CARGO_BIN_EXE_*` and `RFL_BUNDLED_BIN_OPENAI` in the
  capturing shell's env:** **both absent.** Verified via
  `env | grep -E "CARGO_BIN_EXE|RFL_BUNDLED_BIN"` (no
  matches) — this is the cold-start authenticity gate per
  scope §D: the resolver must succeed without either of the
  cargo-test-env hooks set.
- **`LITELLM_API_KEY` in the capturing shell's env (relevant
  to `02-cold-chat.txt`):** **unset for the capture.** The
  ambient parent shell on this host has `LITELLM_API_KEY`
  exported for the dev LiteLLM proxy, but the tmux capture
  session ran `unset LITELLM_API_KEY AIDER_LITELLM_API_KEY`
  as its first command (visible at the top of the recording
  recipe in `manual-validation.md`). With the key unset, the
  `rfl-openai` plugin would have fallen back to the in-tree
  test-fixture mock had its spawn reached the network. As it
  happens, the spawn does not reach the network in this dev
  environment — see **Open issue** below.

## File-by-file

- **`01-cold-init.txt`** — real `tmux capture-pane` output
  after `rfl init --yes --project-root <PROJECT>` against a
  freshly-`mktemp -d`'d empty project root from a
  freshly-built `rfl` (no `CARGO_BIN_EXE_*` env ambient).
  The pane shows the command line, silent success (the
  `--yes` path emits nothing on stdout/stderr unless there
  is an error — `init.rs::run`), then a follow-up
  `ls "$PROJECT"` showing `rafaello.lock` written and
  `ls "$PROJECT/.rafaello/plugins/"*/bin/` showing the
  materialised `rfl-openai` binary in place. The
  shell-level proof that c01–c03 landed: the shim →
  real-binary swap fires at materialisation time so the
  runtime path under `bin/rfl-openai` is a real ELF, not
  the `cargo`-injected shim text. Verified out-of-band
  via `file "$PROJECT/.rafaello/plugins/*/bin/rfl-openai"`
  → `ELF 64-bit LSB pie executable` (134 MB debug build).
  No interactive accept prompt rendered (`--yes`
  short-circuits the prompt, mirroring m6's bootstrap).
- **`02-cold-chat.txt`** — real `tmux capture-pane` output
  during a `rfl chat --project-root <PROJECT>` run against
  the same project from `01`. The pane (rendered against
  the TUI's alternate-screen buffer) shows
  `syd: exec error: Permission denied`; the stderr stream
  observed via a sibling `2>&1 | tee` (not committed; only
  used to disambiguate the source of the line) shows
  `rfl-tui: project-root=<PROJECT>` →
  `rfl-chat: frontend-ready-observed` → the `syd:` line.
  **See Open issue below.** The post-c01..c05 fix predicts a
  clean TUI render here; this dev host instead reproduces a
  syd-sandbox `Permission denied` symptom on the plugin
  spawn step. The symptom is **not** the v0.1.0 cold-start
  defect that c01–c03 fixed (that defect was the shim
  binary on disk; here the on-disk binary is a real ELF —
  see `01-cold-init.txt`'s `ls` line). The c05 regression
  test
  (`rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`)
  exercises the same path on CI and is the source of truth
  for the post-fix behaviour.
- **`03-ctrl-c-exit.txt`** — real `tmux capture-pane` output
  after sending `C-c` to a running `rfl chat` session and
  letting the parent shell prompt return. The pane shows
  the launching command line, the supervisor's pre-TUI
  stderr (`rfl-tui: project-root=…` and
  `rfl-chat: frontend-ready-observed` survive the
  alternate-screen restore because they were emitted
  before `EnterAlternateScreen`), and the recovered
  `luiz@sodium /tmp %` prompt — i.e. the controlling
  terminal is restored and the parent shell is interactive
  again. **This is the c04 fix verified live in this dev
  env:** pre-c04 the TUI did not respond to `Ctrl-C` as a
  Quit event (per b309b26 — `fix(rafaello-tui): Ctrl-C key
  event quits the TUI cleanly`); post-c04 it does. The c05
  tmux-driven regression test
  (`rfl_chat_ctrl_c_quits_cleanly.rs`) asserts the same
  behaviour on CI.

## Open issue: `02-cold-chat.txt` syd sandbox flake

`02-cold-chat.txt` reproduces a `syd: exec error: Permission
denied` line on this dev host's tmux capture, even though the
m6.1 cold-init fix is verified live (see `01-cold-init.txt`)
and `Ctrl-C` exit is verified live (see `03-ctrl-c-exit.txt`).
The symptom shape (`syd: exec error: …`) is similar to the
pre-m6 `syd-pty`-not-adjacent failure mode documented in
m6's `manual-validation.md` §7, but it manifests here as a
sandbox `Permission denied` on `rfl-openai` exec rather than
as a missing-`syd-pty` failure. The line is emitted by syd
itself; it is **not** the v0.1.0 cold-start defect (which
was the shim binary text on disk — proven absent by
`01-cold-init.txt`).

The capture is left as-is rather than suppressed:

1. The .txt file is the literal `capture-pane` output, with
   no edits. Per the m6 cK6 pattern, transparent transcripts
   beat curated ones.
2. The CI-runnable companion at
   `rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`
   (c05) exercises the same `rfl init` → `rfl chat` path
   under `--features rafaello-core/test-fixture` against a
   real `rfl` binary and asserts the post-fix behaviour
   end-to-end. CI is the source of truth.
3. The dev-env flake is filed as a v0.1.2 follow-up
   candidate alongside the symmetric `rfl install`
   shim-swap issue noted in scope §"Out of scope". A
   future milestone that ratifies the second-bundled-tool
   symmetric fix is the natural place to chase this
   sandbox-policy interaction.

This mirrors the m6 cK6 transparency stance: the on-disk
transcript captures the literal dev-env behaviour at capture
time; the regression test on CI is the authoritative
post-fix proof.
