# Codex

Pilot's `Codex` driver targets the [codex-cli](https://github.com/openai/codex) — OpenAI's headless `codex` agent CLI. Install it from the upstream repo (or via `npm install -g @openai/codex` if you prefer the npm distribution), make sure `codex --version` works on your `PATH`, and pilot will spawn it for you on each turn.

## CLI compatibility

Unlike the Claude Code CLI, codex-cli does not embed its version in the JSON stream, so pilot pins the version in a sidecar metadata file rather than at the top of every fixture. The current pin lives at `tests/fixtures/codex/.metadata.json`:

```
$ cat tests/fixtures/codex/.metadata.json
{
  "cli": "codex-cli",
  "version": "0.130.0",
  ...
}
```

Pilot's replay tests (`tests/fixtures/recorded/codex_*.jsonl`) feed these captures back through the driver to verify event parsing. If you upgrade the CLI past this pin, re-record the fixtures before relying on them as a regression baseline.

## Quick start

```rust
use pilot::{Codex, Session, TurnOptions};
use futures_util::StreamExt;

# async fn run() -> pilot::Result<()> {
let mut session = Session::new(Codex::new(), "/path/to/workdir");
let mut stream = session.send("hello", TurnOptions::default()).await?;
while let Some(item) = stream.next().await {
    // match on pilot::TurnItem
}
# Ok(()) }
```

The driver spawns `codex exec --json --sandbox <mode> --skip-git-repo-check <prompt>` for the first turn and rewrites the trailing prompt as `resume <thread_id> <prompt>` for follow-up turns once a `thread.started` event has been observed. See `src/driver/codex.rs` for the exact argv composition.

## Argv at a glance

A first-turn `Codex::new().command(session_id, "hello", TurnOptions::default())` produces:

```
codex exec --json --sandbox read-only --skip-git-repo-check hello
```

Optional fields slot in between `--skip-git-repo-check` and the trailing prompt:

- `--model <name>` when `TurnOptions::model` or `CodexConfig::default_model` is set
- `-c key=value` for every entry in `CodexConfig::config_overrides`
- `-c reasoning.effort=<low|medium|high>` when `TurnOptions::reasoning` is set
- `--add-dir <dir>` repeated for each entry in `CodexConfig::additional_dirs`
- Anything in `TurnOptions::extra_args`, appended verbatim before the prompt

On follow-up turns the prompt positional is replaced with `resume <thread_id> <prompt>` and the rest of the argv is preserved. The snapshot tests at the bottom of `src/driver/codex.rs` (`default_command_argv_snapshot`, `observe_captures_thread_id_for_resume`) lock this shape in.

## Configuration: `CodexConfig`

Source: `src/driver/codex.rs::CodexConfig`. All fields are public; the struct is `#[non_exhaustive]` and implements `Default`.

| Field                 | Type                    | Default                | Purpose                                                                                                            |
|-----------------------|-------------------------|------------------------|--------------------------------------------------------------------------------------------------------------------|
| `binary`              | `Option<PathBuf>`       | `None` (uses `codex`)  | Override the path to the `codex` executable. Useful for pinned installs or testing against a fork.                 |
| `auth`                | `Auth`                  | `Auth::Ambient`        | Authentication mode — see [Authentication](#authentication).                                                       |
| `default_model`       | `Option<String>`        | `None`                 | Sent as `--model` when `TurnOptions::model` is unset. A per-turn `TurnOptions::model` always wins.                 |
| `sandbox`             | `SandboxMode`           | `SandboxMode::ReadOnly`| Maps to `--sandbox` — see [Sandbox modes](#sandbox-modes).                                                         |
| `skip_git_repo_check` | `bool`                  | `true`                 | Pass `--skip-git-repo-check`. Codex refuses to run outside a git repo by default and pilot is headless, so on.     |
| `config_overrides`    | `Vec<(String, String)>` | empty                  | Each pair emits `-c key=value` to codex's TOML config system.                                                      |
| `extra_env`           | `Vec<(String, String)>` | empty                  | Extra environment variables merged into every spawned child. `TurnOptions::env` is appended after these.           |
| `paths`               | `AgentPaths`            | empty                  | `paths.config_home` is exported as `CODEX_HOME` for the child process.                                             |
| `additional_dirs`     | `Vec<PathBuf>`          | empty                  | Extra read/write roots passed as repeated `--add-dir` flags.                                                       |
| `state`               | `CodexPilotState`       | empty                  | `state.thread_store_path` enables persisting captured thread ids so resume survives process restarts.              |

`AgentPaths`, `Auth`, and `TurnOptions` are defined in `src/driver.rs` and re-exported at the crate root. `SandboxMode` and `CodexPilotState` live in `src/driver/codex.rs` and are re-exported at the crate root.

## Default behavior

- **Auth:** `Auth::Ambient`. The driver does not set `OPENAI_API_KEY`; the spawned `codex` process inherits whatever credentials the user already configured (keychain login, env var, etc.).
- **Sandbox:** `SandboxMode::ReadOnly`. Codex won't write to disk by default — files written during a turn happen only if the config opts into `WorkspaceWrite` or `DangerFullAccess`. This is a meaningful departure from the Claude driver, which does no sandboxing of its own.
- **`skip_git_repo_check`:** `true`. Codex refuses to run outside a git repository unless this flag is passed; pilot turns it on by default so that headless drivers can target arbitrary working directories.
- **Model:** With `default_model = None` and no per-turn override, the CLI selects whichever default the installed `codex` version ships with.

## Sandbox modes

`SandboxMode` is a small enum in `src/driver/codex.rs` that maps directly onto the CLI's `--sandbox` flag.

| Variant                          | CLI value             | Effect                                                                                       |
|----------------------------------|-----------------------|----------------------------------------------------------------------------------------------|
| `SandboxMode::ReadOnly`          | `read-only`           | Codex can read the workdir but writes are blocked. This is the default.                      |
| `SandboxMode::WorkspaceWrite`    | `workspace-write`     | Writes inside the workdir are allowed; writes outside the workdir are blocked.               |
| `SandboxMode::DangerFullAccess`  | `danger-full-access`  | No sandbox; codex can read and write anywhere the child process can reach.                   |

Opt in via `Codex::with_config`:

```rust
use pilot::{Codex, CodexConfig, SandboxMode};

let codex = Codex::with_config(CodexConfig {
    sandbox: SandboxMode::WorkspaceWrite,
    ..Default::default()
});
```

Security caveat: `DangerFullAccess` removes pilot's main safety rail for codex — the agent can write anywhere the child can reach, so reserve it for sandboxed or otherwise expendable working directories.

## Approval flow

Codex maintains an approval gate that is independent of the sandbox: even with `WorkspaceWrite`, the CLI can still pause to ask a human to confirm individual tool calls. In a non-interactive pilot session there is no human at the prompt, so for the recorded tool-use fixture pilot passes `--dangerously-bypass-approvals-and-sandbox` via `TurnOptions::extra_args` to let the write happen during recording (see `tests/recorded_scenarios.rs::codex_tool_use_writes_file_and_emits_toolcall_toolresult`).

Pilot does not (yet) expose `--dangerously-bypass-approvals-and-sandbox` as a typed configuration knob — it is surfaced through `TurnOptions::extra_args` for callers that explicitly opt in. Treat the flag the same way you would treat `BypassPermissions` on Claude: only reach for it in sandboxed or otherwise expendable working directories.

## Authentication

`Auth` lives in `src/driver.rs` and is shared across all drivers.

- `Auth::Ambient` (the default): pilot adds no auth-related env vars. The `codex` child inherits whatever the user is already logged in with — typically a keychain entry from `codex login`, or an `OPENAI_API_KEY` set in the parent shell.
- `Auth::ApiKey(SecretString)`: pilot sets `OPENAI_API_KEY` in the child's environment to the provided secret. `SecretString` redacts on `Debug` so the key won't leak through log lines (see `tests::apikey_auth_injects_openai_api_key_without_leaking_to_debug` in `src/driver/codex.rs`).

```rust
use pilot::{Codex, CodexConfig, Auth};
use secrecy::SecretString;

let ambient = Codex::new();

let explicit = Codex::with_config(CodexConfig {
    auth: Auth::ApiKey(SecretString::from("sk-codex-...".to_string())),
    ..Default::default()
});
```

## Known quirks

- **`--skip-git-repo-check` is on by default.** Codex's vanilla behavior is to refuse to run outside a git repository. Pilot flips the flag on by default so non-git workdirs work out of the box; set `skip_git_repo_check: false` in `CodexConfig` to restore the upstream behavior.
- **Tool calls span two events.** Codex emits `item.started` and `item.completed` events for each tool. The driver maps the `command_execution` and `file_change` item subtypes to `Event::ToolCall` on `item.started` and `Event::ToolResult` on `item.completed`. A `command_execution` whose `exit_code` is non-zero becomes a `ToolResult { ok: false, .. }`; `file_change` always completes as `ok: true` with an empty `output`.
- **`error` and `turn.failed` are split.** A standalone `{"type":"error"}` event maps to a synthetic `Event::AssistantText` carrying the CLI's error message; the trailing `{"type":"turn.failed"}` maps to `Event::TurnComplete { ok: false }`. Together they give callers an error string plus a terminal status, matching the shape Claude produces on `is_error: true`.

## Event mapping

The driver's `parse` method (`src/driver/codex.rs`) consumes one JSON object per line from the CLI's stream and produces zero or more `pilot::Event` values. The mapping:

| Codex JSON                                                              | Pilot `Event`                                          |
|-------------------------------------------------------------------------|--------------------------------------------------------|
| `{ "type": "item.started", "item": { "type": "command_execution" } }`   | `Event::ToolCall { call_id, name: "command_execution", args }` |
| `{ "type": "item.started", "item": { "type": "file_change" } }`         | `Event::ToolCall { call_id, name: "file_change", args }`       |
| `{ "type": "item.completed", "item": { "type": "agent_message" } }`     | `Event::AssistantText { delta }`                       |
| `{ "type": "item.completed", "item": { "type": "command_execution" } }` | `Event::ToolResult { call_id, ok, output }`            |
| `{ "type": "item.completed", "item": { "type": "file_change" } }`       | `Event::ToolResult { call_id, ok: true, output: "" }`  |
| `{ "type": "error", "message": "..." }`                                 | `Event::AssistantText { delta: "..." }`                |
| `{ "type": "turn.failed", ... }`                                        | `Event::TurnComplete { ok: false }`                    |
| `{ "type": "turn.completed", "usage": { ... } }`                        | `Event::Usage { input_tokens, output_tokens }` then `Event::TurnComplete { ok: true }` |
| anything unrecognized                                                   | `Event::Raw { driver: "codex", value }`                |

The `thread.started` event does not produce an `Event` itself — `Driver::observe` captures the `thread_id` so the next turn can issue `codex exec ... resume <thread_id>`.

## Recorded scenarios

These JSONL captures live under `tests/fixtures/recorded/` and feed the replay tests. `cat` them to see the exact stream the CLI produced:

- `tests/fixtures/recorded/codex_happy_path_says_hi.jsonl` — a minimal greeting turn with no tool use. Useful as a sanity check for parser plumbing.
- `tests/fixtures/recorded/codex_invalid_model_yields_failed_turn_complete.jsonl` — invokes the CLI with a bogus `--model` and shows how the driver surfaces the resulting error as a `TurnComplete { ok: false }` with the CLI's error text as preceding `AssistantText`.
- `tests/fixtures/recorded/codex_tool_use_writes_file_and_emits_toolcall_toolresult.jsonl` — a tool-use round-trip recorded with `--dangerously-bypass-approvals-and-sandbox`, where codex issues a `file_change` and a follow-up `command_execution` and pilot parses both `ToolCall` / `ToolResult` pairs.

## Further reading

- Source: [`src/driver/codex.rs`](../src/driver/codex.rs) — driver impl, parser, and unit tests.
- Trait and shared types: [`src/driver.rs`](../src/driver.rs) — `Driver`, `Auth`, `AgentPaths`, `TurnOptions`, `TurnInput`.
- Interactive example: [`examples/chat.rs`](../examples/chat.rs) — terminal chat UI; run with `cargo run --example chat -- --agent codex`.
- Pilot's overall architecture and the cross-driver feature matrix: [`README.md`](../README.md).
- codex-cli repository: <https://github.com/openai/codex>.
