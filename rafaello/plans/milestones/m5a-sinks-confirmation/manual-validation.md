# m5a manual validation

Companion to `scope.md` §"Manual validation". CI cannot exercise the
LiteLLM proxy (`LITELLM_API_KEY` is not present in CI); the headline
integration test uses `rfl-openai-stub` instead. This file records the
manual validation runs that exercise the real proxy and the interactive
surfaces (TUI keys, slash commands, install-time refusal, audit log)
against the operator's machine.

Concrete command lines, URLs, and captured output land during Phase 3
manual runs, following the m4 retrospective pattern. Until then each
section is a step-list scaffold.

## 1. Real-network demo (LiteLLM proxy + `send-mail`)

Runs `rfl chat` against the dev LiteLLM proxy with a fixture lock
setting `RFL_OPENAI_ENDPOINT_URL`, `RFL_OPENAI_MODEL`,
`RFL_OPENAI_API_KEY_ENV = "LITELLM_API_KEY"`, and
`env.pass = ["LITELLM_API_KEY"]`. The bundled `rfl-openai` manifest's
`[capabilities.default.env].allow_secrets` covers `LITELLM_API_KEY` so
the scrubber honours it without `flags.i_know_what_im_doing`; `rfl
status` shows yellow "explicit secret LITELLM_API_KEY", not red
`[OVERRIDE]`.

Steps:

1. Prepare a fixture lock with the env keys above and the
   `mailcat` plugin enabled.
2. Export `LITELLM_API_KEY` in the operator shell.
3. Run `rfl chat` against the fixture lock. (Command line: TBD Phase 3.)
4. Type: `please email alice@example.com that I'll be late`.
5. Observe the model proposes `send-mail`; the confirmation overlay
   fires.
6. Press `y`. Observe the mailcat plugin's on-disk log gains an entry.
7. Repeat steps 3-5 in a fresh session; press `n`. Observe the deny
   path (no mailcat entry; model receives the refusal turn).

## 2. Slash-command demo (`/grant`, `/grants list`, `/revoke`)

Within the same chat session:

1. Type `/grant send-mail to=alice@example.com`. Observe the
   `core.session.command_result` confirmation.
2. Type `/grants list`. Observe the grant entry with its id.
3. Re-issue the same user message from §1 ("please email
   alice@example.com…"). Observe no modal fires (the gate matches
   the grant; the call passes through).
4. Type `/revoke <id>` with the id from step 2. Observe confirmation.
5. Re-issue the same user message. Observe the modal fires again.

## 3. Trifecta refusal demo (`rfl install` / `rfl status`)

1. Prepare a fixture manifest declaring all three trifecta dimensions
   (network reach + secret access + sink class).
2. Run `rfl install <fixture>`. Observe the typed refusal error.
3. Re-run with `rfl install <fixture> --i-know-what-im-doing`. Observe
   install succeeds.
4. Run `rfl status`. Observe the red ANSI `[OVERRIDE]` marker on the
   trifecta plugin row.

## 4. macOS CI URL capture

Per the m4 retrospective pattern, the post-merge driver sweep runs
the workspace on macOS CI. Record the green run URL here once Phase 3
lands.

- Run URL: _TBD Phase 3._

## 5. TUI keyboard interaction walkthrough

A short interactive walk asserting every documented key drives the
expected answer. For each key, fire a confirmation overlay (via the
`send-mail` flow from §1 or an equivalent fixture trigger) and press
the key:

1. `y` — accept this call.
2. `a` — accept and grant going forward (matches §UG `MatchSpec`).
3. `Enter` — same as `y` (documented default).
4. `n` — deny this call.
5. `d` — deny and remember (negative grant).
6. `Esc` — dismiss / treat as deny.
7. `s` — show details / expand argument view.

For each, record the observed `core.session.confirm_answered` event
and the resulting downstream behaviour (passthrough, hold, reply).

## 6. Audit-log inspection

After a session that exercised §1-§5:

1. Locate the session's `audit_events` table (path TBD Phase 3 — the
   AL writer determines the on-disk location).
2. Dump the table.
3. Assert the rows match the operator's actions in order: each
   confirmation prompt, each `y`/`a`/`n`/`d`/`Esc` answer, each
   `/grant` and `/revoke`, and each install-time refusal / override.
