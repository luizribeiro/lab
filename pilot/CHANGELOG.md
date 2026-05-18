# Changelog

All notable changes to `pilot` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial public API: `Session`, `TurnStream`, `Driver`, `Event`, `TurnItem`,
  `Turn`, `TurnInput`, `TurnOptions`, `Error`, `ParseError`, `Auth`,
  `CommandSpec`, `AgentPaths`, `ReasoningLevel`, plus per-driver configs.
- Built-in drivers: `Claude`, `Codex`, `Gemini`, `Pi`.
- Examples: `greeting`, `multi_turn`, `with_api_key`, `with_paths`.
- `test_support::Cassette` driver wraps any inner driver and switches
  between record and replay modes based on fixture-file presence and
  `PILOT_RECORD`/`PILOT_NO_RECORD` env vars. In Replay mode pilot's
  normal spawn+parse pipeline reads the fixture via `cat <path>`
  (Unix-only).
- `cassette!(inner)` macro auto-derives the fixture path from the
  calling test function's name: `tests/fixtures/recorded/<test_fn>.jsonl`.
- Recorded happy-path scenarios for all four drivers (claude, codex,
  gemini, pi). Each test sends a single short prompt and asserts pilot
  normalizes the response into at least one `AssistantText` delta and
  a successful `TurnComplete`.
- Recorded invalid_model scenarios for codex and gemini (claude already
  covered). Per-driver behavior is pinned; future CLI regressions
  surface as test failures. pi's invalid_model test is `#[ignore]`d:
  the pi CLI exits silently with no stream-json on invalid `--model`.
- Recorded tool_use scenarios for all four drivers. Each test asks
  the CLI to write a small file via its file-writing tool and asserts
  pilot saw `ToolCall` + `ToolResult` + `AssistantText` events. codex
  requires `--dangerously-bypass-approvals-and-sandbox` and gemini
  requires `--yolo` at record time to bypass approval gates.

### Changed
- All four built-in drivers (claude, codex, gemini, pi) graduate to
  **stable** status. Each has fixture coverage for greeting + tool-use,
  a `.metadata.json` pinning the CLI version, and a documented
  error-handling path. Pi's silent-error limitation is documented
  in its module rustdoc.
- `TurnStream::cancel().await` now surfaces errors encountered during
  channel drain via the new `Turn::errors: Vec<Error>` field. Previously
  these were silently dropped. The field is empty for natural
  completion via `TurnItem::Complete`.
- `Session::new` and `Session::resume` now take an owned `Driver` value
  (`D: Driver + 'static`) instead of `Arc<dyn Driver>`. The Arc is
  constructed internally. Callers no longer need to import `Arc` or
  `Driver` for basic use.

### Fixed
- `test_support::DefaultSanitizer` now matches both the raw and canonical
  forms of `$HOME`, `$TMPDIR`, and the current working directory.
  Previously, macOS canonical paths (`/private/var/folders/...`) leaked
  through unsanitized.

### Removed
- `test_support::RecordingDriver` (replaced by `test_support::Cassette`).
- `test_support::recorded_test::run_or_replay` and related helpers
  (`mode_for`, `ScenarioMode`) — replaced by the `cassette!()` macro.
- `TurnStream::with_timeout` from the public API. Use
  `TurnOptions.timeout` when calling `Session::send`. The internal
  method is now `pub(crate)`.
- `pilot::driver(name)` factory and `Error::UnknownAgent` variant. Users
  pick the driver by typed constructor (`Claude::new()`, etc.) and
  dispatch via their own match if the agent name comes from CLI args
  or config.

### Lifecycle guarantees
- `TurnStream` Drop kills the child; `cancel().await` returns partial Turn.
- Cross-Session lock prevents concurrent turns on the same `(driver, uuid)`.
- `Turn::final_text()` accumulates `Event::AssistantText` deltas across drivers.
