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

### Removed
- `pilot::driver(name)` factory and `Error::UnknownAgent` variant. Users
  pick the driver by typed constructor (`Claude::new()`, etc.) and
  dispatch via their own match if the agent name comes from CLI args
  or config.

### Lifecycle guarantees
- `TurnStream` Drop kills the child; `cancel().await` returns partial Turn.
- Cross-Session lock prevents concurrent turns on the same `(driver, uuid)`.
- `Turn::final_text()` accumulates `Event::AssistantText` deltas across drivers.
