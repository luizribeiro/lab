# Gemini

Pilot's `Gemini` driver targets Google's [gemini-cli](https://github.com/google-gemini/gemini-cli) — the headless `gemini` agent CLI. Install it from the upstream repo (or `npm install -g @google/gemini-cli` if you prefer the npm distribution), make sure `gemini --version` works on your `PATH`, and pilot will spawn it for you on each turn.

## CLI compatibility

Like codex-cli, gemini-cli does not embed its version in the JSON stream, so pilot pins the version in a sidecar metadata file rather than at the top of every fixture. The current pin lives at `tests/fixtures/gemini/.metadata.json`:

```
$ cat tests/fixtures/gemini/.metadata.json
{
  "cli": "gemini",
  "version": "0.42.0",
  ...
}
```

Pilot's replay tests (`tests/fixtures/recorded/gemini_*.jsonl`) feed these captures back through the driver to verify event parsing. If you upgrade the CLI past this pin, re-record the fixtures before relying on them as a regression baseline.

## Quick start

```rust
use pilot::{Gemini, Session, TurnOptions};
use futures_util::StreamExt;

# async fn run() -> pilot::Result<()> {
let mut session = Session::new(Gemini::new(), "/path/to/workdir");
let mut stream = session.send("hello", TurnOptions::default()).await?;
while let Some(item) = stream.next().await {
    // match on pilot::TurnItem
}
# Ok(()) }
```

The driver spawns `gemini -p <prompt> --output-format stream-json --session-id <uuid> --approval-mode yolo --skip-trust` for the first turn and rewrites `--session-id <uuid>` to `--resume <uuid>` for follow-up turns. See `src/driver/gemini.rs` for the exact argv composition.

## Configuration: `GeminiConfig`

Source: `src/driver/gemini.rs::GeminiConfig`. All fields are public; the struct is `#[non_exhaustive]` and implements `Default`.

| Field                 | Type                    | Default                  | Purpose                                                                                                                  |
|-----------------------|-------------------------|--------------------------|--------------------------------------------------------------------------------------------------------------------------|
| `binary`              | `Option<PathBuf>`       | `None` (uses `gemini`)   | Override the path to the `gemini` executable. Useful for pinned installs or testing against a fork.                      |
| `auth`                | `Auth`                  | `Auth::Ambient`          | Authentication mode — see [Authentication](#authentication).                                                             |
| `default_model`       | `Option<String>`        | `None`                   | Sent as `--model` when `TurnOptions::model` is unset. A per-turn `TurnOptions::model` always wins.                       |
| `approval_mode`       | `ApprovalMode`          | `ApprovalMode::Yolo`     | Maps to `--approval-mode` — see [Approval modes](#approval-modes).                                                       |
| `skip_trust`          | `bool`                  | `true`                   | Pass `--skip-trust` to bypass gemini's per-folder trust prompt. See [Default behavior](#default-behavior) for the tradeoff. |
| `extra_env`           | `Vec<(String, String)>` | empty                    | Extra environment variables merged into every spawned child. `TurnOptions::env` is appended after these.                 |
| `paths`               | `AgentPaths`            | empty                    | Reserved for future use; setting `paths.config_home` on this driver currently returns `Error::UnsupportedOption`.        |
| `include_directories` | `Vec<PathBuf>`          | empty                    | Extra read roots passed as a single `--include-directories <a>,<b>,...` argument.                                        |

`AgentPaths`, `Auth`, and `TurnOptions` are defined in `src/driver.rs` and re-exported at the crate root. `ApprovalMode` lives in `src/driver/gemini.rs` and is re-exported at the crate root.

## Default behavior

- **Auth:** `Auth::Ambient`. The driver does not set `GEMINI_API_KEY`; the spawned `gemini` process inherits whatever credentials the user already configured (keychain login, env var, etc.).
- **Trust:** `skip_trust: true`. Pilot is a headless driver, so without `--skip-trust` every `Session::new(Gemini::new(), workdir).send(...)` would block on gemini's per-folder trust prompt in any workdir that hasn't been trusted in an interactive gemini session first. The default trades fail-closed safety for ergonomics; `skip_trust: true` means gemini will read and execute project-level gemini config from the workdir without asking. **Pass only paths you trust to `Session::new(_, workdir)`.** To restore the trust gate, set `skip_trust: false` explicitly on `GeminiConfig`.
- **Approval:** `ApprovalMode::Yolo` — pilot drives gemini headlessly with all approvals bypassed. Tools execute without prompts. `skip_trust: true` unchanged. Pilot does not sandbox; see [docs/sandboxing.md](sandboxing.md) for the recommended approach (lockin / capsa).
- **Model:** With `default_model = None` and no per-turn override, the CLI selects whichever default the installed `gemini` version ships with.

## Approval modes

`ApprovalMode` is a small enum in `src/driver/gemini.rs` that maps directly onto the CLI's `--approval-mode` flag.

| Variant                  | CLI value     | Effect                                                                                          |
|--------------------------|---------------|-------------------------------------------------------------------------------------------------|
| `ApprovalMode::Yolo`     | `yolo`        | Auto-approves every tool call (equivalent to passing `--yolo`). This is pilot's default.        |
| `ApprovalMode::AutoEdit` | `auto_edit`   | Auto-approves file edits. Other sensitive tool calls still prompt.                              |
| `ApprovalMode::Default`  | flag omitted  | CLI uses its normal interactive approval flow. Tool calls will block waiting for confirmation.  |
| `ApprovalMode::Plan`     | `plan`        | Plan-mode: the agent plans but does not execute tool calls.                                     |

To restore approval gating, configure `ApprovalMode::Default` or `AutoEdit` explicitly via `GeminiConfig`. Out-of-process sandboxing should use a dedicated tool — see [docs/sandboxing.md](sandboxing.md).

```rust
use pilot::{Gemini, GeminiConfig, ApprovalMode};

let gemini = Gemini::with_config(GeminiConfig {
    approval_mode: ApprovalMode::Default,
    ..Default::default()
});
```

Security caveat: `Yolo` runs every tool call the agent decides to make — shell, file edits, network — without confirmation, so reserve it for sandboxed or otherwise expendable working directories.

## Authentication

`Auth` lives in `src/driver.rs` and is shared across all drivers.

- `Auth::Ambient` (the default): pilot adds no auth-related env vars. The `gemini` child inherits whatever the user is already logged in with — typically a keychain entry from `gemini auth`, or a `GEMINI_API_KEY` set in the parent shell.
- `Auth::ApiKey(SecretString)`: pilot sets `GEMINI_API_KEY` in the child's environment to the provided secret. `SecretString` redacts on `Debug` so the key won't leak through log lines (see `tests::apikey_auth_injects_env_var_without_leaking_to_debug` in `src/driver/gemini.rs`).

```rust
use pilot::{Gemini, GeminiConfig, Auth};
use secrecy::SecretString;

let ambient = Gemini::new();

let explicit = Gemini::with_config(GeminiConfig {
    auth: Auth::ApiKey(SecretString::from("ai-...".to_string())),
    ..Default::default()
});
```

## Known quirks

- **`skip_trust: true` is the default.** Gemini's vanilla behavior is to prompt before reading or executing any project-level config from an untrusted folder. Pilot flips the flag on by default so headless drivers can target arbitrary workdirs without an interactive prompt blocking the first turn; set `skip_trust: false` in `GeminiConfig` to restore gemini's fail-closed prompt. The tradeoff is documented in the `skip_trust` doc comment in `src/driver/gemini.rs`.
- **Tool calls require approval in `Default` mode.** Without `ApprovalMode::Yolo` (pilot's default, see [Approval modes](#approval-modes)) or `--yolo` in `TurnOptions::extra_args`, gemini will refuse tool calls in a headless session.
- **`tool_use` / `tool_result` events.** Gemini emits `{"type":"tool_use"}` and `{"type":"tool_result"}` events that pilot normalizes to `Event::ToolCall` and `Event::ToolResult` respectively. A `tool_result` whose `status` is anything other than `"success"` becomes `Event::ToolResult { ok: false, .. }`.
- **Errors arrive as a synthetic `AssistantText`.** Gemini wraps error messages in the trailing `{"type":"result", "status":"error", "error":{"message":"..."}}` envelope. The driver surfaces the message as `Event::AssistantText` followed by `Event::TurnComplete { ok: false }`, matching the shape Claude and Codex produce on a failed turn.

## Recorded scenarios

These JSONL captures live under `tests/fixtures/recorded/` and feed the replay tests. `cat` them to see the exact stream the CLI produced:

- `tests/fixtures/recorded/gemini_happy_path_says_hi.jsonl` — a minimal greeting turn with no tool use. Useful as a sanity check for parser plumbing.
- `tests/fixtures/recorded/gemini_invalid_model_yields_failed_turn_complete.jsonl` — invokes the CLI with a bogus `--model` and shows how the driver surfaces the resulting error as a `TurnComplete { ok: false }` with the CLI's error text as preceding `AssistantText`.
- `tests/fixtures/recorded/gemini_tool_use_writes_file_and_emits_toolcall_toolresult.jsonl` — a tool-use round-trip recorded with `--yolo`, where gemini issues a `write_file` tool call and pilot parses both `ToolCall` and `ToolResult`.

## Further reading

- Source: [`src/driver/gemini.rs`](../src/driver/gemini.rs) — driver impl, parser, and unit tests.
- Trait and shared types: [`src/driver.rs`](../src/driver.rs) — `Driver`, `Auth`, `AgentPaths`, `TurnOptions`, `TurnInput`.
- Interactive example: [`examples/chat.rs`](../examples/chat.rs) — terminal chat UI; run with `cargo run --example chat -- --agent gemini`.
- Pilot's overall architecture and the cross-driver feature matrix: [`README.md`](../README.md).
- gemini-cli repository: <https://github.com/google-gemini/gemini-cli>.
