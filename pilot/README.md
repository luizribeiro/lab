# pilot

Drive headless AI coding-agent CLIs (claude, codex, gemini, pi) from Rust over their stream-JSON modes. Observe events turn-by-turn through a unified `Stream`-based API. No tmux, no PTY, no review-loop overhead — just driving.

## Supported agents

| Agent  | CLI flag set | Resume support | Auth env var | Status |
|--------|---|---|---|---|
| claude | `-p --output-format stream-json --verbose --session-id <uuid>` (first) / `--resume <uuid>` (later) | yes | `ANTHROPIC_API_KEY` | first-class |
| gemini | `-p --output-format stream-json --session-id <uuid>` (first) / `--resume <uuid>` (later) | yes | `GEMINI_API_KEY` | first-class |
| pi     | `-p --mode json --session-dir <dir>` (first) / `+ --continue` (later) | yes | `PI_API_KEY` | first-class |
| codex  | `codex exec --json --sandbox read-only --skip-git-repo-check <prompt>` (first) / `+ resume <thread_id> <prompt>` (later) | yes (auto-captured from `thread.started` event via `Driver::observe`) | `OPENAI_API_KEY` | first-class |

## Quick start

```rust
use futures_util::StreamExt;
use pilot::{Event, Session, TurnItem, TurnOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let driver = pilot::driver("claude")?;
    let mut session = Session::new(driver, "./repo");

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
    pub fn new(driver: Arc<dyn Driver>, workdir: impl Into<PathBuf>) -> Self;
    pub fn resume(driver: Arc<dyn Driver>, id: Uuid, workdir: impl Into<PathBuf>) -> Self;
    pub fn id(&self) -> Uuid;
    pub fn workdir(&self) -> &Path;
    pub async fn send(&mut self, input: impl Into<TurnInput>, opts: TurnOptions) -> Result<TurnStream>;
}

pub struct TurnStream;          // impl Stream<Item = Result<TurnItem>>
impl TurnStream {
    pub fn with_timeout(self, duration: Duration) -> Self;
    pub async fn cancel(self) -> Turn;
}

pub enum TurnItem { Event(Event), Complete(Turn) }

pub enum Event {
    AssistantText { delta: String },
    ToolCall      { call_id: String, name: String, args: serde_json::Value },
    ToolResult    { call_id: String, ok: bool, output: String },
    Thinking      { delta: String },
    Usage         { input_tokens: u64, output_tokens: u64 },
    TurnComplete  { ok: bool, final_text: Option<String> },
    Raw           { driver: &'static str, value: serde_json::Value },
}

pub fn driver(name: &str) -> Result<Arc<dyn Driver>>; // built-in factory
```

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

```sh
cargo run --example greeting -- claude
cargo run --example multi_turn -- gemini
PILOT_AGENT_KEY=sk-... cargo run --example with_api_key
cargo run --example with_paths
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

Pre-1.0. Public API may still shift as more drivers are added.

## License

MIT OR Apache-2.0
