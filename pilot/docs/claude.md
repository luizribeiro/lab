# Claude

Pilot's `Claude` driver targets the [Claude Code CLI](https://docs.claude.com/en/docs/claude-code) — the headless `claude` binary that ships with Anthropic's official CLI. Install it with `npm install -g @anthropic-ai/claude-code` (or via the platform installer of your choice), make sure `claude --version` works on your `PATH`, and pilot will spawn it for you on each turn.

## CLI compatibility

The fixture suite pins the Claude Code CLI version that recorded outputs were captured against. The current pin lives at the top of every recorded fixture as `claude_code_version`. As of this writing:

```
$ grep -o 'claude_code_version":"[^"]*' tests/fixtures/recorded/claude_happy_path_says_hi.jsonl | head -1
claude_code_version":"2.1.143
```

Pilot's replay tests (`tests/fixtures/recorded/*.jsonl`) feed these captures back through the driver to verify event parsing. If you upgrade the CLI past this pin, re-record the fixtures before relying on them as a regression baseline.

## Quick start

```rust
use pilot::{Claude, Session, TurnOptions};
use futures_util::StreamExt;

# async fn run() -> pilot::Result<()> {
let mut session = Session::new(Claude::new(), "/path/to/workdir");
let mut stream = session.send("hello", TurnOptions::default()).await?;
while let Some(item) = stream.next().await {
    // match on pilot::TurnItem
}
# Ok(()) }
```

The driver spawns `claude -p --verbose --output-format stream-json --session-id <uuid>` for the first turn and `--resume <uuid>` for every subsequent turn in the same `Session`. See `src/driver/claude.rs` for the exact argv composition.

## Argv at a glance

A first-turn `Claude::new().command(session_id, "hello", TurnOptions::default())` produces:

```
claude -p --verbose --output-format stream-json \
    --session-id <uuid> \
    -- hello
```

Optional fields slot in between `--session-id` and the trailing `--`:

- `--model <name>` when `TurnOptions::model` or `ClaudeConfig::default_model` is set
- `--permission-mode <acceptEdits|bypassPermissions>` for non-default modes
- `--effort <low|medium|high>` when `TurnOptions::reasoning` is set
- `--add-dir <dir> [<dir>...]` when `ClaudeConfig::additional_dirs` is non-empty
- Anything in `TurnOptions::extra_args`, appended verbatim

On follow-up turns, `--session-id <uuid>` is rewritten to `--resume <uuid>` and the rest of the argv is preserved. The two snapshot tests at the bottom of `src/driver/claude.rs` (`default_command_argv_snapshot` and `resume_command_uses_resume_flag_not_session_id`) lock this shape in.

## Configuration: `ClaudeConfig`

Source: `src/driver/claude.rs::ClaudeConfig`. All fields are public; the struct is `#[non_exhaustive]` and implements `Default`.

| Field             | Type                       | Default                | Purpose                                                                                                  |
|-------------------|----------------------------|------------------------|----------------------------------------------------------------------------------------------------------|
| `binary`          | `Option<PathBuf>`          | `None` (uses `claude`) | Override the path to the `claude` executable. Useful for pinned installs or testing against a fork.      |
| `auth`            | `Auth`                     | `Auth::Ambient`        | Authentication mode — see [Authentication](#authentication).                                             |
| `default_model`   | `Option<String>`           | `None`                 | Sent as `--model` when `TurnOptions::model` is unset. A per-turn `TurnOptions::model` always wins.       |
| `permission_mode` | `PermissionMode`           | `PermissionMode::Default` | Maps to `--permission-mode` — see [Permission modes](#permission-modes).                              |
| `extra_env`       | `Vec<(String, String)>`    | empty                  | Extra environment variables merged into every spawned child. `TurnOptions::env` is appended after these. |
| `paths`           | `AgentPaths`               | empty                  | `paths.config_home` is exported as `CLAUDE_CONFIG_DIR` for the child process.                            |
| `additional_dirs` | `Vec<PathBuf>`             | empty                  | Extra read/write roots passed to `--add-dir`. See the [variadic-flag pitfall](#known-quirks).            |

`AgentPaths`, `Auth`, and `TurnOptions` are defined in `src/driver.rs` and re-exported at the crate root.

## Default behavior

- **Auth:** `Auth::Ambient`. The driver does not set `ANTHROPIC_API_KEY`; the spawned `claude` process inherits whatever credentials the user already configured (keychain login, env var, etc.).
- **Sandboxing:** The Claude Code CLI does not sandbox the working directory, and pilot adds no sandboxing of its own. The child has the same filesystem and network reach as the parent process — choose `workdir` accordingly.
- **Approvals:** `PermissionMode::Default` lets the CLI prompt for tool calls. In a non-interactive pilot session there is no human at the prompt, so pilot-driven tool execution typically requires opting into `AcceptEdits` or `BypassPermissions` (next section).
- **Model:** With `default_model = None` and no per-turn override, the CLI selects whichever default the installed `claude` version ships with.

## Permission modes

`PermissionMode` is a small enum in `src/driver/claude.rs` that maps directly onto the CLI's `--permission-mode` flag.

| Variant                              | CLI value             | Effect                                                                                          |
|--------------------------------------|-----------------------|-------------------------------------------------------------------------------------------------|
| `PermissionMode::Default`            | flag omitted          | CLI uses its normal interactive approval flow. Tool calls will block waiting for confirmation.  |
| `PermissionMode::AcceptEdits`        | `acceptEdits`         | Auto-approves file edits. Other sensitive tool calls still prompt.                              |
| `PermissionMode::BypassPermissions`  | `bypassPermissions`   | Disables the approval flow entirely; the agent runs all tool calls without confirmation.        |

Opt in via `Claude::with_config`:

```rust
use pilot::{Claude, ClaudeConfig, PermissionMode};

let claude = Claude::with_config(ClaudeConfig {
    permission_mode: PermissionMode::BypassPermissions,
    ..Default::default()
});
```

Security caveat: `BypassPermissions` runs every tool call the agent decides to make — shell, file edits, network — without confirmation, so reserve it for sandboxed or otherwise expendable working directories.

## Authentication

`Auth` lives in `src/driver.rs` and is shared across all drivers.

- `Auth::Ambient` (the default): pilot adds no auth-related env vars. The `claude` child inherits whatever the user is already logged in with — typically a keychain entry from `claude login`, or an `ANTHROPIC_API_KEY` set in the parent shell.
- `Auth::ApiKey(SecretString)`: pilot sets `ANTHROPIC_API_KEY` in the child's environment to the provided secret. `SecretString` redacts on `Debug` so the key won't leak through log lines (see `tests::apikey_auth_injects_env_var_without_leaking_to_debug` in `src/driver/claude.rs`).

```rust
use pilot::{Claude, ClaudeConfig, Auth};
use secrecy::SecretString;

let ambient = Claude::new();

let explicit = Claude::with_config(ClaudeConfig {
    auth: Auth::ApiKey(SecretString::from("sk-ant-...".to_string())),
    ..Default::default()
});
```

## Known quirks

- **`--add-dir` variadic pitfall.** The Claude Code CLI's `--add-dir DIR...` greedily consumes following arguments, which means a naive `--add-dir /foo "prompt text"` ends up treating the prompt as another directory. Pilot inserts a literal `--` separator between flags and the prompt positional (see `src/driver/claude.rs:117-120`), so the prompt is always unambiguous. Nothing for callers to do; just be aware if you compare pilot's argv against examples in the CLI's docs.
- **Permission-request events.** When running in `PermissionMode::Default`, the CLI emits structured permission-request events on tool calls. The current driver surfaces unrecognized message types as `Event::Raw { driver: "claude", value }` — including these permission prompts. Normalizing them into a dedicated `Event::PermissionRequest` is Phase 3 work; until then, consumers that care can match on `Event::Raw` and parse the `value` themselves.
- **`--session-id` is single-use.** The CLI rejects re-using a session UUID it has already seen with "Session ID is already in use". The driver handles this by switching to `--resume <uuid>` for the second and later turns of a session (`Driver::resume_command` in `src/driver/claude.rs`).

## Event mapping

The driver's `parse` method (`src/driver/claude.rs`) consumes one JSON object per line from the CLI's stream and produces zero or more `pilot::Event` values. The mapping:

| Claude JSON                                          | Pilot `Event`                                          |
|------------------------------------------------------|--------------------------------------------------------|
| `{ "type": "assistant", "message": { content: [text] } }`     | `Event::AssistantText { delta }`                       |
| `{ "type": "assistant", "message": { content: [thinking] } }` | `Event::Thinking { delta }`                            |
| `{ "type": "assistant", "message": { content: [tool_use] } }` | `Event::ToolCall { call_id, name, args }`              |
| `assistant.message.usage`                            | `Event::Usage { input_tokens, output_tokens }`         |
| `{ "type": "user", "message": { content: [tool_result] } }`   | `Event::ToolResult { call_id, ok, output }`            |
| `{ "type": "result", is_error: false }`              | `Event::TurnComplete { ok: true }`                     |
| `{ "type": "result", is_error: true, result: "..." }`| `Event::AssistantText { delta: "..." }` then `Event::TurnComplete { ok: false }` |
| anything unrecognized                                 | `Event::Raw { driver: "claude", value }`               |

Tool-result `content` arrays are flattened by joining all `text` chunks; non-string variants fall back to their JSON `to_string()`. See `stringify_tool_result_content` in `src/driver/claude.rs`.

## Recorded scenarios

These JSONL captures live under `tests/fixtures/recorded/` and feed the replay tests. `cat` them to see the exact stream the CLI produced:

- `tests/fixtures/recorded/claude_happy_path_says_hi.jsonl` — a minimal greeting turn with no tool use. Useful as a sanity check for parser plumbing.
- `tests/fixtures/recorded/claude_invalid_model_yields_failed_turn_complete.jsonl` — invokes the CLI with a bogus `--model` and shows how the driver surfaces the resulting error as a `TurnComplete { ok: false }` with the CLI's error text as preceding `AssistantText`.
- `tests/fixtures/recorded/claude_tool_use_writes_file_and_emits_toolcall_toolresult.jsonl` — a tool-use round-trip where the agent issues a file-writing tool call and pilot parses both the `ToolCall` and the matching `ToolResult`.

## Further reading

- Source: [`src/driver/claude.rs`](../src/driver/claude.rs) — driver impl, parser, and unit tests.
- Trait and shared types: [`src/driver.rs`](../src/driver.rs) — `Driver`, `Auth`, `AgentPaths`, `TurnOptions`, `TurnInput`.
- Interactive example: [`examples/chat.rs`](../examples/chat.rs) — terminal chat UI; run with `cargo run --example chat -- --agent claude`.
- Pilot's overall architecture and the cross-driver feature matrix: [`README.md`](../README.md).
- Claude Code CLI homepage: <https://docs.claude.com/en/docs/claude-code>.
