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

## §5 — Interactive tmux recording (filled by c27)

*Placeholder — filled by c27 (Phase J2).*

c27 lands the concrete tmux-driven recording of an end-to-end
`rfl chat` session against the §1 cold-start bootstrap: launch
in a tmux session, drive a prompt that triggers the
`send-mail` modal, capture the modal text via
`tmux capture-pane`, assert the live overlay substrings (the
` confirm ` border, the `send-mail via local:mailcat@0.0.0 —
sinks: [mail]` summary, the `args: { … "alice@example.com" …
}` line, the `sinks: mail` line), allow via the `a` binding
(`rafaello-tui/src/lib.rs:70`), capture the assistant response,
and quit cleanly via Ctrl-C (the m3 chat-loop quit binding;
round-3 J2 correction).

Per hard requirement #3 this section is the **canonical proof
of life** for v1: no mechanical-coverage substitute is
acceptable.

*Status*: ⏳ pending — filled by c27.

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
`taint_dropped_in_envelope`) alongside m5a's `confirm_denied`
/ `tool_call_started` / `tool_call_completed` family.

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

---

If a LiteLLM-proxy-driven run is included alongside §1 / §2 /
§5 / §G, label it explicitly **"Real-provider walkthrough"** —
the provider is real (LiteLLM proxy) but no fetch / network-
sink tool is exercised on the v1 demo path (m5b's
`rafaello-fetch` remains available in the workspace but is
**not** the m6 demo tool per scope §"Canonical demo tool").
