# pilot

Drive headless AI coding-agent CLIs (claude, codex, gemini, pi) from Rust over their stream-JSON modes. Observe events turn-by-turn through a unified `Stream`-based API. No tmux, no PTY, no review-loop overhead — just driving.

## Supported agents

| Agent  | CLI flag set | Resume support | Auth env var | Status |
|--------|---|---|---|---|
| claude | `-p --verbose --output-format stream-json --session-id <uuid>` (first) / `--resume <uuid>` (later) | yes | `ANTHROPIC_API_KEY` | **stable** |
| codex  | `codex exec --json --sandbox danger-full-access --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check <prompt>` (first) / `+ resume <thread_id> <prompt>` (later) | yes (auto-captured via `Driver::observe`) | `OPENAI_API_KEY` | **stable** |
| gemini | `-p --output-format stream-json --session-id <uuid>` (first) / `--resume <uuid>` (later) | yes | `GEMINI_API_KEY` | **stable** |
| pi     | `-p --mode json --session-dir <dir>` (first) / `+ --continue` (later) | yes | provider-dependent | **stable** (silent-error limitation; see driver docs) |

**What "stable" means:** every driver has fixture coverage for greeting,
tool-use, and (for codex/gemini/claude) error paths, with the underlying
CLI version pinned in `tests/fixtures/<driver>/.metadata.json`. We test
each driver live end-to-end via the e2e smoke suite. Driver-specific
limitations are documented in the rustdoc on each driver's module. Pi's
silent-error behavior is the only one users need to know about — failed
pi turns produce empty assistant content with no distinct error signal.

## Driver reference

Each built-in driver ships with a dedicated docs page covering its argv, configuration, default behavior, event mapping, and known quirks:

- [claude](docs/claude.md) — drives Anthropic's Claude Code CLI.
- [codex](docs/codex.md) — drives OpenAI's codex-cli with a built-in sandbox.
- [gemini](docs/gemini.md) — drives Google's gemini-cli with a per-folder trust gate.
- [pi](docs/pi.md) — drives Inflection's multi-provider pi CLI.

The matrix below summarizes the defaults that most often surprise headless callers — pick a driver and follow its doc link for the full story.

Pilot is a thin transport — defaults are permissive so the smallest example works headlessly. For real workloads where the agent shouldn't have unrestricted access, see [docs/sandboxing.md](docs/sandboxing.md) for the recommended layering (driver config + lockin/capsa).

| Driver | Doc | Default sandbox/approval | Notable defaults | Native restriction knob |
|--------|-----|--------------------------|------------------|-------------------------|
| claude | [docs/claude.md](docs/claude.md) | `PermissionMode::BypassPermissions` — no prompts | No sandboxing; `Auth::Ambient` | `permission_mode: PermissionMode::Default` or `AcceptEdits` |
| codex  | [docs/codex.md](docs/codex.md)  | `SandboxMode::DangerFullAccess` + `dangerously_bypass_approvals: true` — full access | `skip_git_repo_check: true`; `Auth::Ambient` | `sandbox: SandboxMode::ReadOnly` and `dangerously_bypass_approvals: false` |
| gemini | [docs/gemini.md](docs/gemini.md) | `ApprovalMode::Yolo` — no prompts | `skip_trust: true` bypasses per-folder trust gate | `approval_mode: ApprovalMode::Default` or `AutoEdit` |
| pi     | [docs/pi.md](docs/pi.md) | Provider-dependent; silent-error on invalid input | No `provider` set by default — set `provider` for reliability | provider-specific (see [docs/pi.md](docs/pi.md)) |

Recorded fixtures under `tests/fixtures/recorded/<driver>_*.jsonl` show each driver's real CLI output for greeting, tool-use, and (where supported) error scenarios — they're the ground truth that the defaults above are measured against.

## Quick start

```rust
use futures_util::StreamExt;
use pilot::{Claude, Event, Session, TurnItem, TurnOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::new(Claude::new(), "./repo");

    let mut stream = session
        .send("audit the codebase", TurnOptions::default())
        .await?;

    while let Some(item) = stream.next().await {
        match item? {
            TurnItem::Event(Event::AssistantText { delta }) => print!("{delta}"),
            TurnItem::Event(Event::ToolCall { name, .. })   => eprintln!("[tool: {name}]"),
            TurnItem::Complete(turn) => eprintln!("\n[done, {} events]", turn.events.len()),
            _ => {}
        }
    }
    Ok(())
}
```

Spawn-per-turn is the uniform model: every `send()` spawns a fresh child of the underlying CLI, the CLI persists conversation state on disk keyed by `Session::id()`, and continuity across turns falls out for free. Drop the `TurnStream` to kill the child; call `cancel().await` to recover whatever partial events arrived.

## Public API at a glance

```rust
pub struct Session;
impl Session {
    pub fn new<D: Driver + 'static>(driver: D, workdir: impl Into<PathBuf>) -> Self;
    pub fn resume<D: Driver + 'static>(driver: D, id: Uuid, workdir: impl Into<PathBuf>) -> Self;
    pub fn id(&self) -> Uuid;
    pub fn workdir(&self) -> &Path;
    pub async fn send(&mut self, input: impl Into<TurnInput>, opts: TurnOptions) -> Result<TurnStream>;
}

pub struct TurnStream;          // impl Stream<Item = Result<TurnItem>>
impl TurnStream {
    pub async fn cancel(self) -> Turn;
}
// Per-turn timeout is configured via `TurnOptions.timeout` passed to `send()`.

pub enum TurnItem { Event(Event), Complete(Turn) }

pub enum Event {
    AssistantText { delta: String },
    ToolCall      { call_id: String, name: String, args: serde_json::Value },
    ToolResult    { call_id: String, ok: bool, output: String },
    Thinking      { delta: String },
    Usage         { input_tokens: u64, output_tokens: u64 },
    TurnComplete  { ok: bool },
    Raw           { driver: &'static str, value: serde_json::Value },
}
```

Construct the driver you want via its typed constructor — `Claude::new()`,
`Codex::new()`, `Gemini::new()`, `Pi::new()`, or the corresponding
`*::with_config(...)` for custom configuration — and pass it directly to
`Session::new`. The session takes the driver by value; the `Arc` used to
share it with the turn-stream is constructed internally.

For the canonical agent response text, use `Turn::final_text()`, which
concatenates all `AssistantText` deltas observed during the turn. Drivers
that don't stream deltas (e.g. claude's error-result path) emit a
synthetic `AssistantText` so this still returns usable text.

The `TurnInput` enum is the input type accepted by `Session::send`. Today
only `Text(String)` exists; future multimodal variants (image, file) can
be added without breaking SemVer because the enum is `#[non_exhaustive]`.

## Examples

Runnable examples live in `examples/`:

| File | What it shows |
|---|---|
| `examples/greeting.rs` | Minimal one-turn invocation against any registered driver. |
| `examples/multi_turn.rs` | Two turns on the same `Session`; the second auto-dispatches through `resume_command()`. |
| `examples/with_api_key.rs` | Explicit `Auth::ApiKey(SecretString)` configuration. |
| `examples/with_paths.rs` | `ClaudeConfig.additional_dirs` + `TurnOptions.timeout`. `AgentPaths::config_home` is shown commented-out (needs auth in the isolated dir). |
| `examples/repl/` | Self-contained workspace crate (`pilot-repl`) — a ratatui inline-viewport chat REPL. The prompt stays visible while turns stream, so you can type ahead and queue prompts (dispatched in order on each `TurnComplete`). Markdown rendering of assistant replies via ratskin/termimad, persistent tool-call status lines, spinner. Esc cancels the in-flight turn (Esc again clears the queue), Ctrl+R reverse-i-search overlay, Ctrl+G to edit the current prompt in `$EDITOR`, Ctrl+D quits. History persists at `~/.pilothistory`, per-session transcripts at `~/.pilot/transcripts/<agent>-<uuid>.jsonl` are replayed on `--resume <uuid>`. The resume command for the current session prints on exit. Uses the terminal's native scrollback — no alt-screen, so you can scroll up through the conversation with your terminal's own scrollbar. Deps live in its own `Cargo.toml`, kept out of pilot's dev-deps. |

```sh
cargo run --example greeting -- claude
cargo run --example multi_turn -- gemini
PILOT_AGENT_KEY=sk-... cargo run --example with_api_key
cargo run --example with_paths
cargo run -p pilot-repl -- --agent claude
cargo run -p pilot-repl -- --agent claude --resume 6e7c…
```

## Design notes

- **Driver trait** (`Driver`): per-CLI implementations. Two command builders — `command()` for the first turn, `resume_command()` for follow-ups, with a default impl that delegates so simple drivers don't have to override.
- **CommandSpec** is structured (`program`, `args`, `env`) so the library owns subprocess lifecycle invariants; drivers only describe what to invoke.
- **Bounded backpressure**: a 256-slot mpsc channel between the reader task and the consumer. Slow consumers stall the child via stdout pipe back-pressure rather than buffering unboundedly.
- **Cancellation**: every terminal path (`Complete`, `Err`, `Timeout`, `cancel()`, `Drop`) releases the underlying `ProcessHandle` immediately. `kill_on_drop(true)` on the tokio command ensures the child gets SIGTERM/SIGKILL even if a caller forgets to drain.
- **First-turn vs resume**: drivers whose CLI uses different flags (gemini, pi) override `resume_command`. `Session` tracks the number of `TurnItem::Complete`s actually yielded by the stream. A stream that ends in an error, a `Timeout`, or never reaches `Complete` does NOT bump the counter — retrying on the same `Session` re-uses `command()`. A turn that yields `Complete` with an agent-reported failure (`Event::TurnComplete { ok: false }`) DOES bump the counter, because the CLI's on-disk session was still established — the next call correctly uses `resume_command()`. Codex auto-generates its thread_id on the first turn and emits it as a `thread.started` event; the codex driver overrides `Driver::observe` to capture that id and reuse it as the positional `resume <thread_id>` argument on subsequent turns.
- **Path overrides**: each `*Config` exposes a `paths: AgentPaths` substruct for CLI options whose semantics are common across drivers (currently just `config_home`, mapped to `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, or `PI_CODING_AGENT_DIR`). Drivers that don't support a given path option return `Error::UnsupportedOption` rather than silently ignoring — e.g. gemini's `paths.config_home`. Driver-specific workspace overrides (no shared abstraction because semantics differ): `ClaudeConfig.additional_dirs` and `CodexConfig.additional_dirs` map to `--add-dir`, while `GeminiConfig.include_directories` maps to `--include-directories`. Pi has no equivalent flag and exposes no such field.
- **Secrets**: API keys are stored in `secrecy::SecretString`. `Debug` of any config redacts the inner value. A `tests/secret_hygiene.rs` integration test scans `tests/fixtures/` for common vendor key prefixes (`sk-`, `AIza`, `ghp_`, etc.) so a fixture refresh can't accidentally commit a real key.

## Testing strategy

- Per-driver unit tests for `command()` argv (snapshot via `expect_test`) and `parse()` (paired `.jsonl` + `.events.snap` fixtures).
- Process-layer tests with a `tests/support/fake_agent` Rust binary that supports `emit`, `exit`, `stderr`, `sleep` script directives.
- Cancellation suite: nine specific scenarios (cancel before any events, after some, after natural completion, with stderr active, slow consumer, etc.).
- `proptest` fuzzing of every driver `parse()` against arbitrary `serde_json::Value` (256 cases each, seeded with realistic discriminator strings).
- E2E smoke tests at `tests/e2e_smoke.rs`, `#[ignore]`'d by default, gated by `PILOT_E2E=1`. Run with:

  ```sh
  PILOT_E2E=1 cargo test --features test-support -- --ignored
  ```

## Status

Pre-1.0 but approaching tag. Public API is stable; all four built-in
drivers have parity in feature coverage. The remaining 1.0 work is
polish (cancellation token, doctests, builder ergonomics) — none of
it breaks existing callers.

## Minimum Supported Rust Version

Currently `1.85.0` (set in `Cargo.toml`). After 1.0, MSRV bumps will be
treated as a **minor** version change (per Cargo team guidance and the
broader ecosystem norm). Users who pin to an older Rust toolchain should
pin their `pilot` minor version too.

## License

MIT OR Apache-2.0

The full license texts live in `LICENSE-MIT` and `LICENSE-APACHE` at the
crate root.
