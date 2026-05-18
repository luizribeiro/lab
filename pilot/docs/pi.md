# Pi

Pilot's `Pi` driver targets `pi` — Inflection's headless `pi` agent CLI, a multi-provider front-end that delegates to a configurable backend (github-copilot, openai-codex, anthropic, google, etc.). Install it from the upstream distribution, make sure `pi --version` works on your `PATH`, and pilot will spawn it for you on each turn.

## CLI compatibility

Like codex-cli and gemini-cli, the pi CLI does not embed its version in the JSON stream, so pilot pins the version in a sidecar metadata file rather than at the top of every fixture. The current pin lives at `tests/fixtures/pi/.metadata.json`:

```
$ cat tests/fixtures/pi/.metadata.json
{
  "cli": "pi (Inflection)",
  "version": "0.73.1",
  ...
}
```

Pilot's replay tests (`tests/fixtures/recorded/pi_*.jsonl`) feed these captures back through the driver to verify event parsing. If you upgrade the CLI past this pin, re-record the fixtures before relying on them as a regression baseline.

## Quick start

```rust
use pilot::{Pi, Session, TurnOptions};
use futures_util::StreamExt;

# async fn run() -> pilot::Result<()> {
let mut session = Session::new(Pi::new(), "/path/to/workdir");
let mut stream = session.send("hello", TurnOptions::default()).await?;
while let Some(item) = stream.next().await {
    // match on pilot::TurnItem
}
# Ok(()) }
```

The driver spawns `pi -p --mode json --session-dir <path> <prompt>` for the first turn and adds `--continue` for every subsequent turn in the same `Session`. See `src/driver/pi.rs` for the exact argv composition.

## Configuration: `PiConfig`

Source: `src/driver/pi.rs::PiConfig`. All fields are public; the struct is `#[non_exhaustive]` and implements `Default`.

| Field           | Type                    | Default                | Purpose                                                                                                                                       |
|-----------------|-------------------------|------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------|
| `binary`        | `Option<PathBuf>`       | `None` (uses `pi`)     | Override the path to the `pi` executable. Useful for pinned installs or testing against a fork.                                               |
| `auth`          | `Auth`                  | `Auth::Ambient`        | Authentication mode — see [Authentication](#authentication).                                                                                  |
| `provider`      | `Option<String>`        | `None`                 | Backend provider (e.g. `github-copilot`, `openai-codex`, `anthropic`, `google`). Sent as `--provider`. See [Providers](#providers).            |
| `default_model` | `Option<String>`        | `None`                 | Sent as `--model` when `TurnOptions::model` is unset. A per-turn `TurnOptions::model` always wins.                                            |
| `extra_env`     | `Vec<(String, String)>` | empty                  | Extra environment variables merged into every spawned child. `TurnOptions::env` is appended after these.                                      |
| `paths`         | `AgentPaths`            | empty                  | `paths.config_home` is exported as `PI_CODING_AGENT_DIR` for the child process.                                                               |
| `state`         | `PiPilotState`          | empty                  | Pilot-managed state. `state.session_root` is the root under which pilot derives a unique per-session subdirectory (see [Session state](#session-state)). |

`AgentPaths`, `Auth`, and `TurnOptions` are defined in `src/driver.rs` and re-exported at the crate root. `PiPilotState` lives in `src/driver/pi.rs` and is re-exported at the crate root.

## Default behavior

- **Auth:** `Auth::Ambient`. The driver does not set `PI_API_KEY`; the spawned `pi` process inherits whatever credentials the user already configured (per-provider login, env var, etc.).
- **Provider:** None set. Pi falls back to whatever provider its local config has selected. That selection is unstable across machines and depends on which `pi login <provider>` flows the user has completed — set `PiConfig.provider` explicitly for headless reliability.
- **Approvals:** Pi has no per-tool approval gate; tools execute without prompts. Pilot does not sandbox; see [docs/sandboxing.md](sandboxing.md) for the recommended approach (lockin / capsa).
- **Model:** With `default_model = None` and no per-turn override, pi's CLI selects whichever default the configured provider ships with.

## Providers

Unlike the other drivers, pi is a multi-provider front-end: each turn delegates to a backend provider that has its own auth flow, model catalog, and tool-call conventions. Pi maps `--provider <name>` onto the backend it spawns.

Commonly available provider names include:

- `github-copilot`
- `openai-codex`
- `anthropic`
- `google`

The exact set depends on the pi CLI version and which providers have been registered locally — `pi --help` is authoritative for what your installed version supports.

```rust
use pilot::{Pi, PiConfig};

let pi = Pi::with_config(PiConfig {
    provider: Some("anthropic".into()),
    ..Default::default()
});
```

Because pi's *default* provider is whatever the local config last selected, leaving `PiConfig.provider` at `None` makes a headless pipeline non-reproducible across machines. Set it explicitly whenever you care which backend handles the turn.

## Authentication

`Auth` lives in `src/driver.rs` and is shared across all drivers.

- `Auth::Ambient` (the default): pilot adds no auth-related env vars. The `pi` child inherits whatever the user is already logged in with via pi's per-provider login flow.
- `Auth::ApiKey(SecretString)`: pilot sets `PI_API_KEY` in the child's environment to the provided secret. `SecretString` redacts on `Debug` so the key won't leak through log lines (see `tests::apikey_auth_injects_pi_api_key_without_leaking_to_debug` in `src/driver/pi.rs`).

```rust
use pilot::{Pi, PiConfig, Auth};
use secrecy::SecretString;

let ambient = Pi::new();

let explicit = Pi::with_config(PiConfig {
    auth: Auth::ApiKey(SecretString::from("pi-...".to_string())),
    ..Default::default()
});
```

Pi's actual auth flow varies by backend provider — anthropic, github-copilot, and google each have their own credential stores. `Auth::ApiKey` injects a single `PI_API_KEY` env var as a best-effort override; whether the configured provider actually consumes it depends on the backend. For providers that read distinct env vars, set those via `PiConfig.extra_env` instead.

## Session state

Pi diverges from claude, codex, and gemini in how it handles session continuity: those CLIs carry session state internally (in-memory or in a CLI-managed store keyed by session id / thread id). Pi requires a per-session **filesystem directory** that the CLI reads and writes for the lifetime of the session.

Pilot derives a unique subdirectory per `Session::id()` under `PiPilotState.session_root` and passes it to pi as `--session-dir <path>`. The default root is `$HOME/.pilot/pi-sessions`; override it by setting `PiConfig.state.session_root` to a directory of your choosing.

This is filesystem state, not in-memory state — it persists across pilot process restarts and survives as long as the directory does. Sessions are isolated from each other by the UUID-named subdirectory; deleting that directory erases the session.

## Known limitations

- **Silent errors on invalid input.** Pi emits no stream-JSON events when given an invalid `--model` (and possibly other malformed args). The child process exits non-zero with no structured error event, so pilot has no message to surface as `Event::AssistantText` and no `TurnComplete { ok: false }` to emit — `TurnComplete.ok` is always `true` for pi turns. Pilot can detect this condition only as a non-zero exit, not as a structured event. The `pi_invalid_model_yields_failed_turn_complete` integration test is `#[ignore]`d with the rationale "pi emits no events on invalid model (silent-error limitation)" (see `tests/recorded_scenarios.rs:45`), and no fixture is committed. If you need robust error handling, validate inputs (especially `--model` values) before sending, or check `Turn::final_text().is_empty()` after the stream ends as a fallback.

- **Tool-call events use a different shape than other agents.** Pi nests tool calls under `assistantMessageEvent.type = "toolcall_end"` inside a `message_update` frame, emits tool results as top-level `tool_execution_end` events, and uses `thinking_delta` inside `assistantMessageEvent` for chain-of-thought. Pilot normalizes all three into `Event::ToolCall`, `Event::ToolResult`, and `Event::Thinking` respectively — see the `parse` impl in `src/driver/pi.rs` for the mapping.

## Recorded scenarios

These JSONL captures live under `tests/fixtures/recorded/` and feed the replay tests. `cat` them to see the exact stream the CLI produced:

- `tests/fixtures/recorded/pi_happy_path_says_hi.jsonl` — a minimal greeting turn with no tool use. Useful as a sanity check for parser plumbing.
- `tests/fixtures/recorded/pi_tool_use_writes_file_and_emits_toolcall_toolresult.jsonl` — a tool-use round-trip where the agent issues a bash tool call and pilot parses both the `ToolCall` and the matching `ToolResult`.

There is no `pi_invalid_model_yields_failed_turn_complete.jsonl`: the corresponding test in `tests/recorded_scenarios.rs` is `#[ignore]`d because of the silent-error limitation described above — pi produces no events to pin, so there is nothing useful to record.

## Further reading

- Source: [`src/driver/pi.rs`](../src/driver/pi.rs) — driver impl, parser, and unit tests. The module-level doc comment formally documents the silent-error limitation.
- Trait and shared types: [`src/driver.rs`](../src/driver.rs) — `Driver`, `Auth`, `AgentPaths`, `TurnOptions`, `TurnInput`.
- Interactive example: [`examples/chat.rs`](../examples/chat.rs) — terminal chat UI; run with `cargo run --example chat -- --agent pi`.
- Pilot's overall architecture and the cross-driver feature matrix: [`README.md`](../README.md).
