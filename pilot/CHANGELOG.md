# Changelog

All notable changes to `pilot` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Documentation
- Marked codex/gemini/pi drivers as experimental in the README. Claude
  is the only driver with stable schema/parse coverage at this point.

### Added
- Initial public API: `Session`, `TurnStream`, `Driver`, `Event`, `TurnItem`,
  `Turn`, `TurnInput`, `TurnOptions`, `Error`, `ParseError`, `Auth`,
  `CommandSpec`, `AgentPaths`, `ReasoningLevel`, plus per-driver configs.
- Built-in drivers: `Claude`, `Codex`, `Gemini`, `Pi`.
- Examples: `greeting`, `multi_turn`, `with_api_key`, `with_paths`.

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

### Removed
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
