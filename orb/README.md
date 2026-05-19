# orb

Terminal workspace for agentic coding sessions.

Orb is the app layer on top of [pilot](../pilot/). It drives agent CLIs from a
native terminal UI with live prompt composition, streaming replies, persistent
transcripts, queued prompts, and resume support.

## Run

```sh
cargo run -- --agent claude
```

Resume a previous session:

```sh
cargo run -- --agent claude --resume <uuid>
```

Orb stores app-local state under `~/.orb/`:

- `~/.orb/history` prompt history
- `~/.orb/transcripts/<agent>-<uuid>.jsonl` transcripts
- `~/.orb/codex-threads.json`
