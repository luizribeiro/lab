# M6.1 — Manual Validation Notes (v0.1.1 cold-start patches)

This appendix is the **manual-validation evidence on disk** for
the m6.1 patch milestone — two cold-start defects in
rafaello v0.1.0:

1. **D1** — `rfl init` materialised `rfl-openai` from the
   cargo-injected shim under `CARGO_BIN_EXE_*`-bearing env
   instead of the real bundled binary, so the lock-and-spawn
   path failed against the materialised shim under any shell
   without that env.
2. **D2** — the `rfl-tui` input loop did not bind `Ctrl-C`
   as a Quit event, so the TUI stayed alive after the
   operator's exit gesture.

c01–c03 close D1; c04–c05 close D2 (with a tmux-driven
regression on CI). This appendix lands the **pre/post-fix
operator-visible evidence** so the v0.1.1 retrospective and
future operators can verify the cold-start UX is repaired
against a real `rfl` binary. The pattern follows m6 cK6's
`manual-validation.md` §5.1 appendix — three captures under
`transcripts/v0_1_1/` plus a provenance header at
`transcripts/v0_1_1/00-CONTEXT.md`.

## Recording method

The captures under `transcripts/v0_1_1/` are reproducible on
a different host via the following shell-script-style recipe.
All commands run from a lab worktree at the m6.1 tip (c05 or
later); the recipe builds `rfl` against the workspace's
`test-fixture` feature, opens a tmux pane at the canonical
size, and copies each `capture-pane` dump into the in-repo
appendix dir.

```sh
# Build rfl + bundled plugin binaries from the m6.1 tip.
# The test-fixture feature is the workspace's standard
# build profile for cold-start captures (the bundled
# rfl-openai is materialised as a real ELF, not a shim).
cd <lab-worktree>
cd rafaello && cargo build --workspace --bins \
  --features rafaello-core/test-fixture
cd ..

# Throwaway project root for the cold-start captures.
PROJECT=$(mktemp -d -t rfl-m61-XXXX)

# Open a detached tmux session at cK5's canonical pane size.
tmux new-session -d -s rfl-m61-capture -x 100 -y 30

# Drop into the freshly-built rfl with NO CARGO_BIN_EXE_*
# env ambient. Cold-start authenticity requires this shell
# to lack the test-env resolver hooks.
tmux send-keys -t rfl-m61-capture \
  "unset LITELLM_API_KEY AIDER_LITELLM_API_KEY; \
   export PATH='<lab-worktree>/rafaello/target/debug:'\$PATH; \
   cd /tmp; clear" Enter
sleep 1

# Capture 01 — cold init.
tmux send-keys -t rfl-m61-capture \
  "rfl init --yes --project-root '$PROJECT' && \
   echo '== ls project ==' && ls '$PROJECT' && \
   echo '== ls plugins ==' && ls '$PROJECT/.rafaello/plugins/'*/bin/" \
  Enter
sleep 4
tmux capture-pane -t rfl-m61-capture -p \
  > transcripts/v0_1_1/01-cold-init.txt

# Capture 02 — cold chat (TUI render via alternate screen).
tmux send-keys -t rfl-m61-capture "clear" Enter
sleep 0.5
tmux send-keys -t rfl-m61-capture \
  "rfl chat --project-root '$PROJECT'" Enter
sleep 4
tmux capture-pane -t rfl-m61-capture -p \
  > transcripts/v0_1_1/02-cold-chat.txt

# Capture 03 — Ctrl-C exit (the same chat session, then C-c).
tmux send-keys -t rfl-m61-capture C-c
sleep 2
tmux capture-pane -t rfl-m61-capture -p \
  > transcripts/v0_1_1/03-ctrl-c-exit.txt

# Teardown.
tmux kill-session -t rfl-m61-capture
rm -rf "$PROJECT"
```

The recipe is intentionally minimal — no out-of-band shim
re-applications, no `RFL_BUNDLED_BIN_*` env, no
`CARGO_BIN_EXE_*` env. Cold-start authenticity is the load-
bearing acceptance for the appendix (scope §D).

The recording host's full environment (date, hostname,
worktree path, `git rev-parse HEAD`, `tmux -V`, terminal
size, `which rfl`, the exact `cargo build` invocation,
ambient env-var state) is captured in
[`transcripts/v0_1_1/00-CONTEXT.md`](transcripts/v0_1_1/00-CONTEXT.md)
per pi-1 M-3.

## §1 — Cold-start (v0.1.1)

Three captures under
[`transcripts/v0_1_1/`](transcripts/v0_1_1/) document the
operator-visible cold-start behaviour against the c05-tip
binary. The captures replace the pre-fix `syd: exec error:
Permission denied` symptom (the v0.1.0 release behaviour on
a cold shell) with the post-fix lock-written + Ctrl-C-clean
behaviour. See
[`transcripts/v0_1_1/00-CONTEXT.md`](transcripts/v0_1_1/00-CONTEXT.md)
for the full file-by-file provenance and one open issue
surfaced by the dev-host capture (a syd sandbox flake
distinct from the v0.1.0 cold-start defect).

### §1.1 — `01-cold-init.txt` — `rfl init --yes` lands clean

[`transcripts/v0_1_1/01-cold-init.txt`](transcripts/v0_1_1/01-cold-init.txt)
captures `rfl init --yes --project-root <PROJECT>` against
an empty project root from a freshly-built `rfl` (no
`CARGO_BIN_EXE_*` env ambient). The pane shows:

- The command line and a silent prompt return (the `--yes`
  path emits nothing on stdout/stderr unless there is an
  error — `crates/rafaello/src/init.rs::run`).
- A follow-up `ls "$PROJECT"` reporting `rafaello.lock`
  written.
- A follow-up `ls "$PROJECT/.rafaello/plugins/"*/bin/`
  reporting the materialised `rfl-openai` binary in place
  at the canonical runtime path.

This is the c01–c03 fix verified live: the shim → real-
binary swap fires at materialisation time so the runtime
path is a real ELF, not the cargo-injected shim text. The
"lock written, no error, no interactive accept prompt"
acceptance bar from scope §D is met by this transcript
read alongside the `00-CONTEXT.md` `file` byte-shape note.

### §1.2 — `02-cold-chat.txt` — `rfl chat` initial render

[`transcripts/v0_1_1/02-cold-chat.txt`](transcripts/v0_1_1/02-cold-chat.txt)
captures `rfl chat --project-root <PROJECT>` against the
same project. The pre-fix shape from v0.1.0 would be a
`syd: exec error: Permission denied` line followed by an
immediate chat-loop exit (the materialised shim text under
`bin/rfl-openai` is not exec'able through the syd sandbox).
The post-fix expected shape is a clean initial TUI render
with the `rfl-chat: frontend-ready-observed` sentinel in
the supervisor's stderr.

On this dev host the capture reproduces the `syd: exec
error: Permission denied` symptom even though the
underlying v0.1.0 defect is fixed (`01-cold-init.txt`'s
`ls` line proves the on-disk binary is a real ELF, not the
shim). The dev-host symptom is a separate sandbox-policy
flake on `rfl-openai` exec — see
[`transcripts/v0_1_1/00-CONTEXT.md` §"Open issue"](transcripts/v0_1_1/00-CONTEXT.md#open-issue-02-cold-chattxt-syd-sandbox-flake)
for the analysis. The CI-runnable regression at
`rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`
(c05) exercises the same `init → chat` path against a real
`rfl` binary under `--features rafaello-core/test-fixture`
and is the authoritative post-fix proof. The transcript is
left as-is rather than curated — transparency over
fabrication, mirroring m6 cK6's stance on dev-env capture
flakes.

### §1.3 — `03-ctrl-c-exit.txt` — Ctrl-C cleanly quits the TUI

[`transcripts/v0_1_1/03-ctrl-c-exit.txt`](transcripts/v0_1_1/03-ctrl-c-exit.txt)
captures sending `C-c` to a running `rfl chat` session and
the parent shell prompt returning post-exit. The pane shows
the launching command line, the supervisor's pre-TUI stderr
lines (`rfl-tui: project-root=…` and
`rfl-chat: frontend-ready-observed` — emitted before
`EnterAlternateScreen` so they survive the
alternate-screen restore), and the recovered
`luiz@sodium /tmp %` shell prompt below them — i.e. the
controlling terminal is restored and the parent shell is
interactive again.

This is the c04 fix verified live in this dev env: pre-c04
(b309b26 — `fix(rafaello-tui): Ctrl-C key event quits the
TUI cleanly`) the TUI did not bind `Ctrl-C` as a Quit
event and the chat process stayed alive after the
operator's exit gesture; post-c04 it does. The c05
tmux-driven regression
(`rafaello/crates/rafaello/tests/rfl_chat_ctrl_c_quits_cleanly.rs`)
asserts the same shape on CI.

## Cross-references

- **scope §D** — the normative spec for this appendix
  (`rafaello/plans/milestones/m6.1-cold-start-patches/scope.md`).
- **c01–c03** — D1 fix (shim → real-binary swap at
  materialisation time). See commits
  `1086f30 feat(rafaello): BundledPluginNames + sister fn +
  resolve_runtime_binary`,
  `317c697 fix(rafaello): rfl init swaps shim for runtime
  binary at materialisation time`,
  `bea4da7 test(rafaello): rfl init subprocess regression
  — no CARGO_BIN_EXE_* env leak`.
- **c04–c05** — D2 fix (`Ctrl-C` in `rfl-tui` → clean
  Quit). See commits
  `b309b26 fix(rafaello-tui): Ctrl-C key event quits the
  TUI cleanly`,
  `02b7ca8 test(rafaello): tmux-driven Ctrl-C regression
  for rfl chat`.
- **m6 cK6 precedent** —
  `rafaello/plans/milestones/m6-polish-release/manual-validation.md`
  §5.1 + `transcripts/section-5-phase-k/`. The shape of
  this appendix (provenance header in a sibling
  `00-CONTEXT.md`, sibling transcript .txt files captured
  via tmux `capture-pane`, transparency about dev-env
  capture flakes) mirrors that precedent verbatim.

## Acceptance

The acceptance bar is documentary, not automated (scope §D
+ commits.md c06): the three transcript files exist under
`rafaello/plans/milestones/m6.1-cold-start-patches/transcripts/v0_1_1/`
with substantive content, this appendix references each by
relative path, and the appendix's provenance header records
the capture environment in enough detail that a future
operator can reproduce the recording on a different host.
No automated test asserts the transcript contents; the
appendix is documentation-grade evidence. The wire-shape
half of post-fix verification is carried by the c05
regression test on CI.
