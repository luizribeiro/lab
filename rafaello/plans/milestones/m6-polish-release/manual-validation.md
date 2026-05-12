# M6 — Manual Validation Notes

The operator-facing surface for the **rafaello-v0.1** release.
Each section is a manual-run walkthrough recorded against the
m6 build, in ratification order — an operator can walk §1 → §G
sequentially and end with a usable, audited rafaello install.

Scope anchors: §J1 (this skeleton), §J2 (the §5 tmux recording,
filled by c27), m5a / m5b §5 row 11 carryover. Hard requirement
#3 is the load-bearing acceptance for §5; hard requirements #1
and #4 land against §1 / §2; hard requirements #2 and #5 land
against §7.

## §1 — `rfl chat` cold-start walkthrough

Per hard requirement #1 + #4: a new operator with the lab repo
checked out runs a ≤5-line shell sequence inside
`nix develop .#rafaello --impure --command …` and lands in a
functioning interactive `rfl chat` against the dev LiteLLM
proxy. **No** `env CARGO_BIN_EXE_syd-pty=…` workaround. **No**
`export PATH=/nix/store/…` workaround. **No** hand-crafted
lock.

Run the canonical bootstrap (scope §"Hard requirements" #4):

```sh
cd ~/your/project
nix develop .#rafaello --impure --command rfl init
export LITELLM_API_KEY=…           # dev-proxy key from pass
nix develop .#rafaello --impure --command rfl install rfl-mailcat
nix develop .#rafaello --impure --command rfl chat
```

Expected post-`rfl init` state:

- `./rafaello.lock` exists with the bundled `rfl-openai` entry
  pre-installed against the dev LiteLLM endpoint (`decisions.md`
  row 38; `overview.md` §8.1).
- `./.rafaello/state/session.sqlite` exists (m5a §2.4 pinned the
  path); `audit_events` is empty.
- `compile::resolve_entry` succeeds against the post-`rfl init`
  lock without manual edits.

Expected post-`rfl chat` state:

- TUI renders the initial pane; the bundled `rfl-openai`
  provider is reachable via the dev LiteLLM proxy; no
  `setup_pty` error fires.
- Typing a benign prompt yields an assistant response from the
  live model.

*Status*: ⏳ pending — runs once the m6 build reaches the merge
candidate per the driver post-merge sweep.

## §2 — `rfl install rfl-mailcat` walkthrough

Per Phase B's positional-arg install UX. The canonical demo
tool is **`rafaello-mailcat`** (canonical id
`local:mailcat@0.0.0`, declaring the tool `send-mail` with
`sinks = ["mail"]` per the live m5a fixture lock at
`rafaello/fixtures/m5b-locks/rafaello.lock`).

Invocation shape (positional arg, no `--name=` flag):

```sh
nix develop .#rafaello --impure --command \
  rfl install rfl-mailcat --project-root "$PROJECT"
```

Expected post-install state:

- `./rafaello.lock` grows a `local:mailcat@0.0.0` plugin entry
  with the `send-mail` tool declared (`sinks = ["mail"]`).
- The plugin binary is materialised under the Nix store and
  reachable via the bundled supervisor's `LoadPolicy::Lazy`
  spawn path (no eager subprocess on install).
- `rfl chat` now exposes `send-mail` as an available tool; the
  modal fires on first invocation with the live overlay copy at
  `rafaello-tui/src/confirm.rs:160-211` (the c16 `provenance:`
  block render).

*Status*: ⏳ pending — runs immediately after §1.

## §3 — Wire-shape note (preserved from m5b c15)

- `core.session.confirm_request` `details.taint` is serialised
  as `Vec<TaintEntry>` — a JSON array of `{source, detail?}`
  objects (`detail` omitted when `None`). When the inbound
  `core.session.tool_request` envelope has no taint, the array
  renders as `[]` (an empty array), never `null`. §CD1 / §CD3
  (m5b scope).

The m6 build preserves this wire shape unchanged; no
m6-introduced surface alters the taint serialisation.

## §4 — macOS CI run URL

The driver post-merge sweep recorded against the GitHub Actions
macOS workflow (m5a §5 row 9 / m5b §5 row 9 carryover hard
gate).

*Status*: ⏳ pending — placeholder URL until the first green
m6 merge candidate. After push, paste the workflow run URL
here, e.g.:

    https://github.com/luizribeiro/lab/actions/runs/<id>

The URL must point at a run whose `m6 / macOS` job is green
against the rafaello-v0.1 candidate commit.

## §5 — Interactive tmux recording

The tmux-driven end-to-end recording of an `rfl chat` session
against the §1 cold-start bootstrap, captured against
`rfl-openai-stub` (deterministic; the canonical proof-of-life
per hard requirement #3) and copied into
`rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`.
See §J below for the 2026-05-12 LiteLLM outage that pinned the
stub-driven capture as the only landed evidence.

### Tmux script (verbatim from scope §J2)

```sh
# Round-4 B-2 fold: every `rfl <subcommand>` invocation runs
# from inside the lab worktree (where `.#rafaello` resolves)
# and points at the throwaway PROJECT via `--project-root`.

LAB_WORKTREE=/home/luiz/lab            # or current rafaello-v0.1 worktree
PROJECT=$(mktemp -d -t m6-demo-XXXX)
TRANSCRIPTS="$PROJECT/transcripts/section-5"
mkdir -p "$TRANSCRIPTS"

cd "$LAB_WORKTREE"

# Materialise the lock + bundled rfl-openai under $PROJECT.
nix develop .#rafaello --impure --command \
  rfl init --yes --project-root "$PROJECT"

export LITELLM_API_KEY=<dev-proxy-key-from-pass>

# Install rfl-mailcat under the same $PROJECT.
nix develop .#rafaello --impure --command \
  rfl install rfl-mailcat --project-root "$PROJECT"

# Open the tmux session that hosts rfl chat against $PROJECT.
tmux new-session -d -s rafaello-m6-demo \
  "cd '$LAB_WORKTREE' && nix develop .#rafaello --impure --command \
     rfl chat --project-root '$PROJECT'"

# Wait for the TUI to render the initial pane.
sleep 2
tmux capture-pane -t rafaello-m6-demo -p \
  > "$TRANSCRIPTS/01-after-launch.txt"

# Send a prompt that triggers the send-mail tool.
tmux send-keys -t rafaello-m6-demo \
  "Please email alice@example.com a one-line hello note." Enter
sleep 3
tmux capture-pane -t rafaello-m6-demo -p \
  > "$TRANSCRIPTS/02-modal.txt"
# Live overlay copy (confirm.rs:160-211):
#   - title border:  " confirm "
#   - summary line:  "send-mail via local:mailcat@0.0.0 — sinks: [mail]"
#   - args line:     "args: { … "alice@example.com" … }"
#   - sinks line:    "sinks: mail"
grep " confirm "          "$TRANSCRIPTS/02-modal.txt"
grep "send-mail via"      "$TRANSCRIPTS/02-modal.txt"
grep "sinks: mail"        "$TRANSCRIPTS/02-modal.txt"
grep "alice@example.com"  "$TRANSCRIPTS/02-modal.txt"

# Allow the call. Live binding (rafaello-tui/src/lib.rs:70):
#   KeyCode::Char('y') | KeyCode::Char('a') | KeyCode::Enter => Allow.
tmux send-keys -t rafaello-m6-demo "a"
sleep 3
tmux capture-pane -t rafaello-m6-demo -p \
  > "$TRANSCRIPTS/03-response.txt"
# The assistant-message line from the live LiteLLM model (or the
# multi-turn stub if running deterministically) acknowledges the
# send. Operator pastes the rendered line; no fixed substring
# asserted because real-model wording is non-deterministic.

# Quit cleanly. The m3 chat loop's quit binding is Ctrl-C
# (the TUI's input-mode loop doesn't bind 'q' as quit — see
# rafaello-tui/src/lib.rs input handling). Owner-judgment item
# 12 confirms the live binding at implementation time.
tmux send-keys -t rafaello-m6-demo "C-c"
sleep 1
tmux kill-session -t rafaello-m6-demo

# Audit + SQLite dumps.
nix develop .#rafaello --impure --command \
  rfl audit --project-root "$PROJECT" \
  > "$TRANSCRIPTS/04-audit.txt"
grep "confirm_request"   "$TRANSCRIPTS/04-audit.txt"
grep "confirm_allowed"   "$TRANSCRIPTS/04-audit.txt"

sqlite3 "$PROJECT/.rafaello/state/session.sqlite" \
  "SELECT seq, kind, request_id FROM audit_events ORDER BY seq" \
  > "$TRANSCRIPTS/05-sqlite-audit.txt"
sqlite3 "$PROJECT/.rafaello/state/session.sqlite" \
  "SELECT seq, kind FROM entries ORDER BY seq" \
  > "$TRANSCRIPTS/06-sqlite-entries.txt"

# Round-4 B-2: final copy of captured transcripts into the
# in-repo committed location.
REPO_TRANSCRIPTS="$LAB_WORKTREE/rafaello/plans/milestones/m6-polish-release/transcripts/section-5"
mkdir -p "$REPO_TRANSCRIPTS"
cp "$TRANSCRIPTS"/*.txt "$REPO_TRANSCRIPTS/"
```

### Captured transcripts

The files live under
`rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`.
Retro round 2 replaced the c27 schematic content (which pi-1 §B1
correctly flagged as fabricated against the live
`AuditKind::as_str()` table) with real captures from a
`nix build .#rafaello`-built `rfl` running against a long-lived
Python `http.server` OpenAI stub. See `00-CONTEXT.md` in the
same directory for the full capture environment, the
file-by-file provenance, and the two open issues surfaced by
the capture (the supervisor `env_clear` regression on
`CARGO_BIN_EXE_syd-pty`; the headless-tmux `capture-pane`
limitation that left 01-03 blank).

- `00-CONTEXT.md` — capture context, file-by-file provenance,
  open issues route.
- `01-after-launch.txt` — real `tmux capture-pane` after
  spawning `rfl chat …` in detached tmux. **Blank** because
  the cloud-agent harness's headless tmux does not surface
  ratatui's alternate-screen render; the chat process is alive
  and `wire-events.txt` shows the bus events firing. Operator
  re-capture against an attached tmux is a post-merge
  follow-up.
- `02-modal.txt` — real `capture-pane` after the prompt is
  typed. Same blank-pane caveat as 01.
- `03-response.txt` — real `capture-pane` after the `a`
  Allow keybinding fires. Same blank-pane caveat as 01.
- `04-audit.txt` — real `rfl audit --project-root <PROJECT>`
  dump. Four rows: `install_accepted` (from the `rfl install
  rfl-mailcat` step), `confirm_request`,
  `confirm_request_taint_attached`, `confirm_allowed`. Live
  `AuditKind::as_str()` variants only (pi-1 §B1 + §N1
  closure). Request id `01KRDW85BVC348Y5XS50RR6D77` is a real
  ULID. Bracketed `[<rid>|-]` rendering per
  `crates/rafaello/src/audit_cli.rs:232-233`.
- `05-sqlite-audit.txt` — real `sqlite3` dump of
  `SELECT seq, kind, request_id FROM audit_events ORDER BY
  seq`. Same four rows as 04.
- `06-sqlite-entries.txt` — real `sqlite3` dump of
  `SELECT seq, kind FROM entries ORDER BY seq`. Three rows:
  `text` (the user-message entry), `tool_call`, `tool_result`.
- `wire-events.txt` — real `rfl-tui` stderr from the same
  chat run. Each `bus.event topic=…` line is a real bus
  broadcast observed by the TUI bridge; the
  `user_message → tool_request → confirm_request →
  confirm_reply → tool_result` sequence is end-to-end
  evidence that the wire shape ran against the real
  binaries.

### Grep expectations

The `rfl audit` and `sqlite3` substrings are asserted
against the populated dumps:

| File | grep substring | source |
| --- | --- | --- |
| `04-audit.txt` | `confirm_request` | row 2 |
| `04-audit.txt` | `confirm_allowed` | row 4 |
| `05-sqlite-audit.txt` | `confirm_request` | row 2 |
| `05-sqlite-audit.txt` | `confirm_allowed` | row 4 |
| `06-sqlite-entries.txt` | `tool_call` | row 1 |
| `06-sqlite-entries.txt` | `tool_result` | row 2 |
| `wire-events.txt` | `frontend-ready-observed` | line 2 |
| `wire-events.txt` | `core.session.confirm_request` | line 6 |
| `wire-events.txt` | `core.session.tool_result` | line 9 |

The scope §J2 TUI-render substring grep set (`" confirm "`,
`"send-mail via"`, `"sinks: mail"`, `"alice@example.com"`)
remains the **operator-witnessed acceptance criterion**.
Those substrings are not present in the headless 01-03
captures; they fire only in an attached tmux render. The
post-merge driver sweep replaces 01-03 with the live render
and re-asserts the full grep set. Until then, the wire-event
table above + `04-audit.txt` rows 2 + 4 are the round-2
evidence that the same flow ran end-to-end against the live
binaries.

### Quit binding

`Ctrl-C` terminates the chat loop (the m3 chat-loop quit
binding; round-3 J2 correction over round 2's mistaken `q`
assumption — `rafaello-tui/src/lib.rs`'s input-mode handler
does not bind `q` as quit). Owner-judgment item 12 records the
implementation-time verification gate; the live binding stands
as-is for v1.

### Programmatic companion

The regression-grade companion that exercises the same flow
deterministically lives at
`rafaello/crates/rafaello/tests/rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`.
It uses `rfl-openai-stub` (scope §A8 multi-turn contract,
mirrored via an in-process listener for the longer plugin-
spawn-and-validate window the integrated flow needs) plus
`RFL_TUI_TEST_CONFIRM_ANSWERS=allow` (m5b §5 row 56 hook) to
drive `init → install → chat → confirm → persist` end-to-end
inside a temp `--project-root`. The test asserts:

- the `entries` table holds the canonical `tool_call` +
  `tool_result` + assistant-message text rows;
- the `audit_events` table holds the `confirm_request` +
  `confirm_allowed` rows;
- the chat process exits with status zero — the test-mode
  analogue of the manual `Ctrl-C`-clean exit (the live TUI's
  quit binding is `Ctrl-C` per the round-3 J2 correction; the
  test-mode chat exits after the scripted turns drain and the
  `RFL_TUI_MAX_LIFETIME` budget is honoured).

The CI-runnable test plus the round-2 real captures under
`transcripts/section-5/` (audit dumps + wire-event log)
satisfy the wire-shape half of hard requirement #3. The
operator-witnessed TUI render half is gated on the
post-merge driver sweep documented in `00-CONTEXT.md`.

*Status*: ◐ partial (round 2). Wire-shape + audit evidence
landed via real captures replacing c27's schematic content;
TUI render capture deferred to attached-tmux operator sweep.

## §6 — Audit-log inspection walkthrough

Per Phase D's new `rfl audit` CLI. Replaces the m5b-era raw
`sqlite3` query against `audit_events`; the same rows are now
surfaced via a typed CLI that reads
`<project_root>/.rafaello/state/session.sqlite` (m5a §2.4
pinned the path).

Concrete invocation:

```sh
rfl audit --project-root <PROJECT> \
  --kind confirm_request --kind confirm_allowed
```

Expected output shape (one row per matching event; the c11
formatter — `<seq>  <at>  <kind>  <request_id>  <payload-
summary>`):

    <seq>  <at>  <kind>  <request_id>  <payload-summary>

Concretely, after the §5 walkthrough's allow-arm trajectory:

    42  2026-05-12T18:03:21Z  confirm_request  req-7a3f…  tool=send-mail sinks=[mail]
    43  2026-05-12T18:03:24Z  confirm_allowed  req-7a3f…  via=keybinding:a

Operator assertions:

- The two rows share a `request_id`.
- `<seq>` is monotonic across the session.
- `<at>` is RFC 3339 UTC.
- The `--kind` flag filters by kind; passing it twice unions
  the matches (the c11/c12 CLI shape).

Cross-reference: an unfiltered dump (`rfl audit --project-root
<PROJECT>`) also surfaces the m5b kinds
(`confirm_request_taint_attached`,
`plugin_publish_rejected_taint_superset`,
`tool_request_taint_unioned_from_in_reply_to`) alongside m5a's
`confirm_denied` / `confirm_allowed_with_session_grant` /
`grant_added` family. The authoritative kind list is
`AuditKind::as_str()` at
`rafaello-core/src/audit/mod.rs:64-91`; pi-1 round-1 §N1
correctly noted that the c27-era prose mentioned
`taint_dropped_in_envelope` / `tool_call_started` /
`tool_call_completed`, none of which exist in the live enum.

*Status*: ⏳ pending — runs after §5 against the same
`$PROJECT`.

## §7 — syd-pty failure-mode reproduction + fix verification

Per hard requirement #2 (the fix) + hard requirement #5 (the
"documented for posterity" half). Records the pre-m6
`setup_pty` failure observed on 2026-05-12 against the
m5a-RATIFIED build (owner: `syd-pty` was not adjacent on
`PATH` / `CARGO_BIN_EXE_syd-pty` when `syd` spawned the plugin
subprocess) and the m6 fix that obviates the manual env-var
recipe.

Concrete repro (the **failure** half — pre-m6 build, for
posterity only):

```sh
# In a clean shell, OUTSIDE `nix develop`, against a release
# `rfl chat` binary (e.g. an m5a-RATIFIED tag).
cd ~/your/project
./path/to/release/rfl chat
```

Expected pre-m6 failure: the supervisor's plugin-spawn path
fires a `setup_pty` error — `syd-pty` not adjacent on `PATH`
and `CARGO_BIN_EXE_syd-pty` unset; `syd` cannot construct the
PTY for the plugin subprocess; the chat loop exits before the
first prompt.

Concrete fix verification (the **success** half — m6 build):

```sh
cd ~/your/project
nix develop .#rafaello --impure --command rfl chat
```

Expected post-fix behaviour: the lockin-sandbox materialises
`syd-pty` adjacent to `syd` under the Nix store; the
supervisor's plugin-spawn path finds it without any
`CARGO_BIN_EXE_syd-pty=…` shim; `rfl chat` enters the TUI
cleanly.

The post-m6 release narrative records that the lockin-sandbox
fix **obviates** the manual env-var recipe. Per N-2 framing
(scope hard requirement #5), the user-facing README never
directs current devshell users at the manual recipe; that
recipe lives only as a "temporary workaround for pre-m6
builds" subsection.

*Status*: ⏳ pending — repro runs against a pre-m6 tag for the
failure half; verification runs against the m6 candidate for
the success half.

## §G — Homebrew install smoke

Per the chosen-model **G.β** default (Phase G — separate
`homebrew-rafaello` tap fetching Nix-built tarballs; scope
§"Phase G", round-3 M-1 fold). Owner-judgment item 10
confirms manual validation only — there is no CI workflow that
runs `brew install` itself.

Concrete walkthrough on a clean macOS host:

```sh
brew tap luizribeiro/rafaello
brew install rafaello
rfl init
rfl install rfl-mailcat
rfl chat
```

Expected post-`brew install` state:

- `rfl` is on `PATH` (Homebrew shim under `/opt/homebrew/bin`
  or `/usr/local/bin` per the host's arch).
- `rfl --version` reports the rafaello-v0.1 tag the formula
  pins.
- The bundled `syd-pty` / `syd` binaries are adjacent to `rfl`
  inside the Homebrew Cellar (per the G.β tarball layout); no
  `setup_pty` error fires on `rfl chat`.

Expected post-`rfl init` state: as §1, but resolved against
the Homebrew-installed `rfl` (no `nix develop` prefix).

Expected post-`rfl install rfl-mailcat` state: as §2.

Expected post-`rfl chat` state: as §1; the TUI renders against
the dev LiteLLM proxy via `LITELLM_API_KEY` exported in the
shell.

*Status*: ⏳ pending — runs after the G.β formula + release
tarball land (Phase G commits c19 / c20); the manual run is
the **only** smoke for the `brew install` path per owner-
judgment item 10.

## §J — LiteLLM proxy outage on 2026-05-12 (c27 capture context)

Context for future maintainers reading this file alongside the
`transcripts/section-5/` artifacts: the §5 transcripts were
captured against `rfl-openai-stub` (scope §A8 / §E1 multi-turn
contract), **not** the live `vllm/qwen3.6-27b` model on the
`https://litellm.thepromisedlan.club/v1` LiteLLM proxy.

Owner-confirmed status at c27 commit time (2026-05-12 ~03:10
local):

- `vllm/qwen3.6-27b` — **down** (upstream `sodium:8001`
  unreachable);
- `vllm/gpt-oss-120b` — **down** (upstream `vanadium:8000`
  unreachable);
- `mlx/deepseek-v4-flash` — 500;
- `mlx/qwen2.5-coder-7b` — 500;
- `mlx/qwen3.6-35b` — alive (the only model still serving
  cleanly on the proxy at retro time).

Per the owner's Phase J guidance:
1. The stub-driven transcript is the canonical proof of life
   for the v1 wire-shape contract — deterministic, reproducible
   without external dependency, and faithful to the OpenAI Chat
   Completions wire shape that `rfl-openai` speaks against the
   proxy.
2. A real-LLM transcript against `mlx/qwen3.6-35b` (via
   `RFL_OPENAI_MODEL=mlx/qwen3.6-35b` overriding the bundled
   `vllm/qwen3.6-27b` default) is welcome alongside the stub
   transcript when the proxy is reachable; the `rfl-openai`
   plugin speaks generic OpenAI Chat Completions, so any alive
   model on the proxy works without code changes. None landed
   in c27 because the operator capture window coincided with
   the outage.
3. With LiteLLM partially down at retro time AND the
   stub-driven transcript already covering the v1 wire-shape
   contract, the stub transcripts under
   `transcripts/section-5/` stand as v1-demo-ready evidence
   per the owner's explicit acceptance.

The CI-runnable companion at
`rafaello/crates/rafaello/tests/rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`
exercises the same flow deterministically against the same
stub contract; future maintainers re-running the J2 tmux
script against a healthy LiteLLM proxy can drop the
real-provider transcripts (e.g. `01b-after-launch-litellm.txt`,
`02b-modal-litellm.txt`, …) alongside the stub captures
without invalidating the existing files.

---

If a LiteLLM-proxy-driven run is included alongside §1 / §2 /
§5 / §G, label it explicitly **"Real-provider walkthrough"** —
the provider is real (LiteLLM proxy) but no fetch / network-
sink tool is exercised on the v1 demo path (m5b's
`rafaello-fetch` remains available in the workspace but is
**not** the m6 demo tool per scope §"Canonical demo tool").
