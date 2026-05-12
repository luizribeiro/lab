# m6 retrospective.md round-1 pi review

> Verdict: blocking
> Counts: B/3 M/4 N/2

The round-1 draft has the right high-level structure and it correctly treats lazy-load as the hot m6 surface, but it is not ready to ratify. The largest issue is evidence: the retrospective marks the manual-validation/tmux gate as closed even though the committed artifacts still contain pending placeholders and the §5 transcript files are not credible live `rfl audit`/SQLite output.

The second blocker class is the draft drift text. Rows 59-68 do not collide with existing `decisions.md` rows (the log currently ends at row 58), but several proposed rows are materially wrong against live code. Those rows must be corrected before the drift commit is allowed to append them.

## Blockers

### B1. The §5 tmux transcript is presented as real evidence, but the committed files are schematic/impossible

Scope hard requirement #3 explicitly rejects the m5a-style substitute: `manual-validation.md` §5 must capture a **real tmux-driven interactive `rfl chat` recording** (scope.md:273-276), and the acceptance summary requires `manual-validation.md` operator-witnessed evidence plus transcript files (scope.md:1876-1879). The retrospective marks this ✓ at retrospective.md:93-101 and again maps the acceptance bullet to c27+c28 at retrospective.md:418-420.

The landed transcript artifacts do not meet that bar:

- `transcripts/section-5/01-after-launch.txt:2` uses `/tmp/m6-demo-xxxx`, not an actual `mktemp` path.
- `04-audit.txt` contains `tool_call_started` / `tool_call_completed`, but live `AuditKind::as_str()` has no such variants (`rafaello-core/src/audit/mod.rs:28-90`).
- live `rfl audit` renders request ids as `[{rid}]` (`audit_cli.rs:232-233`), while `04-audit.txt` has unbracketed `req-*` and hand-written summaries.
- `manual-validation.md` itself describes these files as the canonical proof at lines 111-119, but the files look authored rather than captured.

Owner's LiteLLM outage guidance can justify **stub-only** evidence; it does not justify non-live/fabricated transcript content. Replace c28's artifacts with an actual tmux capture (stub is fine if documented), or mark this as an open blocking item rather than a shipped hard requirement.

### B2. §4 overclaims acceptance coverage for manual validation, macOS, Homebrew, and LiteLLM

The coverage table says every acceptance bullet maps to a commit (retrospective.md:402-421), including macOS CI, clean Homebrew install, the LiteLLM bootstrap, and `manual-validation.md` §1-§7+§G. Live docs still show placeholders:

- §1 cold-start LiteLLM walkthrough: `Status: pending` (manual-validation.md:52-53).
- §2 install walkthrough: pending (manual-validation.md:82).
- §4 macOS CI URL: placeholder/pending, no run URL (manual-validation.md:96-109).
- §6 audit inspection: pending (manual-validation.md:334-335).
- §7 syd-pty verification: pending (manual-validation.md:383-385).
- §G Homebrew smoke describes expected steps, not witnessed output (manual-validation.md:387-420+).

Scope makes macOS CI a hard ratification gate (scope.md:1847-1851), clean Homebrew install an acceptance bullet (scope.md:1868-1870), and operator-witnessed manual validation mandatory (scope.md:1876-1879). The retrospective must distinguish implemented scaffolding/tests from witnessed release evidence. Do not say “Every roadmap negative landed” / “every acceptance bullet maps” until the pending manual gates are either filled or explicitly re-routed with owner ratification.

### B3. Proposed `decisions.md` rows contain live-code falsehoods

The proposed rows 59-68 are correctly numbered after existing row 58, but several row texts would append inaccurate architecture history:

- **Row 62** says syd-pty failure is a typed `Err(SandboxError::SydPtyNotFound)` (retrospective.md:668-670). Live lockin returns `anyhow::bail!` text from `resolve_syd_pty_path` (`lockin/crates/sandbox/src/lib.rs:246-272`); no `SandboxError` variant exists.
- **Row 64** says scripted-turn exhaustion is a deterministic panic (retrospective.md:691-705). Live stub documents and implements `std::process::exit(1)` on miss/exhaustion (`rfl_openai_stub.rs:19-20`, `:281-294`), and the test is named `exhaustion_exits_deterministically`.
- **Row 65** says `$out/share/rafaello/plugins/<plugin>/` exists “for each of the 8 packages” including `rafaello` and `rafaello-tui` (retrospective.md:707-721). Live `package.nix` builds 8 packages but installs plugin trees only for the six plugin packages; `$out/bin` keeps `rfl` and `rfl-tui` (`rafaello/nix/package.nix:16-33`, `:37-63`).
- **Row 68** says first dispatch matches a tool trigger and implies `tool_to_canonical` participates in routing (retrospective.md:750-778). Live gate calls `ensure_spawned(&dispatch_target)` after parsing the payload's canonical dispatch target (`gate/mod.rs:271-292`); `tool_to_canonical` is written by `register_lazy` but not read outside tests (`supervisor.rs:370-389`, `:401-421`).

These are blockers because the next drift commit will make the rows append-only architecture history.

## Major

### M1. Row 67 cites the wrong allow sites

Row 67 claims allow sites in `crates/rafaello-core/src/agent_loop/` and `crates/rafaello/src/lib.rs` (retrospective.md:737-748). Live non-test `result_large_err` allows are in `bus.rs`, `session/mod.rs`, `supervisor.rs`, `reemit/mod.rs`, and `agent/mod.rs` (`rg result_large_err rafaello/crates/...`). Fix the row before appending.

### M2. Row 63 underspecifies live `--since`

The proposed row lists `--since` as `1h` / `30m` / `24h` (retrospective.md:681-683). Live `parse_since` accepts arbitrary non-negative `<number><m|h|d>` and even the error string advertises `7d` (`audit_cli.rs:93-118`, `:46-47`). Use the live grammar and keep the examples as examples.

### M3. Drift plan misses the overview env-var patch for `RFL_SPAWN_TRACE_LOG`

The prompt's drift list includes the new `RFL_SPAWN_TRACE_LOG` env var. The retrospective adds a glossary entry and row-68 mention (retrospective.md:858-865), but the planned `overview.md` patches only cover bundled provider materialisation, v1 scope cut, and package placement (retrospective.md:780-794). `overview.md` §4.6 is the existing reserved / well-known `RFL_*` env-var index (overview.md:479-530); add a planned patch for the test-observability env var or explicitly justify why it stays out.

### M4. The I1 deviation is misframed as a filename deviation

The open item says c23 replaced the literal `core_tools_list_registered_before_provider_spawn.rs` anchor (retrospective.md:497-511, 905-912). The landed file actually keeps that literal filename under `rafaello/crates/rafaello/tests/core_tools_list_registered_before_provider_spawn.rs` (diff stat), but changes the crate/process shape to a startup-event ordering assertion. Reword as a path/crate + assertion-shape deviation, not a missing literal filename.

## Nits

### N1. `manual-validation.md` §6 names non-live audit kinds

The §6 prose says unfiltered audit output surfaces `taint_dropped_in_envelope`, `tool_call_started`, and `tool_call_completed`-style rows (manual-validation.md:327-332 and transcript `04-audit.txt`). These are not in live `AuditKind::as_str()` (`audit/mod.rs:64-90`). Clean this up with B1/B2.

### N2. Row 66 should distinguish committed formula fixture from the future separate tap

The row says distribution is via `luizribeiro/homebrew-rafaello` whose `Formula/rafaello.rb` fetches tarballs (retrospective.md:723-735). Live m6 only commits `homebrew/rafaello.rb` and uploads a rewritten formula as a release asset (`.github/workflows/rafaello-release.yml:47-59`); publishing it into the separate tap is still an owner action. Say that explicitly so the row does not imply the tap repo changed in m6.

## What's working

- The c01-c28 implementation history is contiguous and matches the ratified phase order.
- The live `rfl install` positional resolver matches row 61's core semantics: clap exactly-one-of plus PP1 copy (`install.rs:41-57`, `:114-149`).
- The `rfl audit --request-id` no-join decision is correctly captured at a high level; live SQL only touches `audit_events` (`audit_cli.rs:126-154`).
- The LiteLLM outage carveout is the right framing direction: stub-only can satisfy hard requirement #3 if the evidence is real and the outage is documented.
