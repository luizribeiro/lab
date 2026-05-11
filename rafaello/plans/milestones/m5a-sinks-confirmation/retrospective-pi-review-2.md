# m5a retrospective.md — pi review round 2

Reviewed round-2 `retrospective.md` at `18abc75`, the diff from `a47a4a2`, and the round-1 findings.

## Blocking findings

### B1. B3 is not fully resolved: two `decisions.md` anchors are concretely wrong

Round 2 added `Refines/Reverses` text, but two anchors point at unrelated rows in the live decision table:

- §7.1 says `env.allow_secrets` refines **row 17** as "the manifest env-scrubbing / `i_know_what_im_doing` line". Live row 17 is **capability scoped bundles accepted, flattened at compile time**. It is not env scrubbing and does not mention `i_know_what_im_doing`.
- §7.4 says `core.tools_list` / `CorePluginService` refines **row 27** as "the supervisor's `PluginSupervisor::new` signature / catalog ownership". Live row 27 is **external UDS-attached frontend principals deferred to v2**.

This was a round-1 blocking area (decision additions must be correctly anchored). The new prose creates false decision-log archaeology. Fix by either naming the correct prior row(s) or explicitly saying the row has no prior decision-row anchor and instead refines the relevant RFC/scope surface.

### B2. §8's TUI keyboard walkthrough has the live key mapping backwards

Round 2 removed the invented `core.session.confirm_answered` topic, but the replacement still gives wrong operator instructions:

- Retrospective §8 says `a` produces the session-grant path and `s` only expands details / produces no answer event.
- Live `rafaello-tui/src/lib.rs` maps `y`, `a`, and `Enter` to `Answer::Allow`; `n`, `d`, and `Esc` to `Answer::Deny`; and **`s` to `Answer::AlwaysAllowSession`**.
- The tests agree (`tui_y_key_publishes_allow_answer.rs`, `tui_s_key_publishes_always_allow_session.rs`).

This would make the manual-validation script record the wrong behaviour. §6.4 also calls timeout part of a "four-arm answer enum"; live TUI answers are only `allow` / `deny` / `always_allow_session` (timeout is a test-driver/no-publish mode and gate deadline outcome, not a frontend answer).

## Major findings

### M1. c38's first missing acceptance test is still over-marked as a "functional equivalent"

§3.1 says `agent_loop_does_not_dispatch_tool_request_directly.rs` is a functional equivalent for `rfl_chat_tool_dispatch_goes_through_gate.rs`. It is only a partial substitute: it proves the agent loop no longer dispatches when no gate is constructed. It does not prove, in the `rfl chat` orchestration, that a tool request reaches the plugin through the gate after passthrough/allow.

The retrospective can keep the substitution narrative, but it should not call this a full functional equivalent unless a positive gate-through-orchestration assertion is named. Route it as a partial coverage substitute or add the missing positive regression anchor to §5.

### M2. §6/§8 still invent an audit kind: `install_override`

The glossary patch sketch and manual-validation bullet list `install_override`. Live `AuditKind` serialises override rows as `trifecta_overridden` and `credential_paths_overridden`; there is no `install_override` string in `audit/mod.rs`. Replace `install_override` with the live kind(s), matching scope §AL.

### M3. §6.1 has two live-surface misstatements in the Stream A patch sketch

Two concrete names in the proposed Stream A patch are not live:

- `CompiledPlugin::sinks()` does not exist; live accessors are `tool_sinks`, `tool_sink_classes`, and `tool_always_confirm`.
- `core.session.confirm_resolved` is not "the re-emit's canonicalised reply". Re-emit canonicalises `frontend.tui.confirm_answer` to `core.session.confirm_reply`; the gate publishes `core.session.confirm_resolved` for resolution/short-circuit visibility.

These are patch-sketch details, but they are exactly the sort of invented stream-drift claim that caused prior retro rounds to fail.

## Non-blocking notes / polish

### N1. Round/status boilerplate is stale

The header still says `at hash TBD-round-2`, and the footer still says "End of m5a retrospective round 1 draft." Update both before owner ratification.

### N2. §6.3 overstates the current overview §4.6 contents

§6.3 says overview §4.6 currently lists `RFL_PROVIDER_ID` and the `RFL_FIXTURE_*` family. The live §4.6 lists core-injected reserved env vars through `RFL_PROVIDER_ID`; it does not list `RFL_FIXTURE_*`. Reword the drift patch to avoid saying the current overview already contains fixture test-driver vars.

### N3. §1's shortstat command is still ambiguous after the retro commits

The text says ``git diff rafaello-v0.1..HEAD --shortstat`` reports the c41-tip stat. At current HEAD it does not; the c41-tip stat is from `git diff rafaello-v0.1..5e36890 --shortstat`. The paragraph has a parenthetical c41 qualifier, but the command should use the c41 hash to avoid a reproducibility trap.

## Round-1 closure check

| Round-1 finding | Status |
|---|---|
| B1 c38 test-name substitution | Partially fixed; deviation documented, but one substitute is overclaimed as a functional equivalent (M1). |
| B2 audit DB path | Fixed; `.rafaello/state/session.sqlite` is now named. |
| B3 drift sweep / decision rows | Partially fixed; drift list expanded, but decision anchors are wrong (B1). |
| M1 c16 validator cache claim | Fixed. |
| M2 test inventory totals | Fixed. |
| M3 production `result_large_err` claim | Fixed. |
| M4 manual-validation names | Partially fixed; confirm topics corrected, but key mapping and audit override kind are wrong (B2/M2). |
| N1 Stream A anchor | Mostly fixed; see M3 for live-surface details. |
| N2 status marker copy | Fixed. |
| N3 `TestHooks` cfg | Fixed. |

## Verdict

Not ready for owner ratification yet. Issues raised: **2 blocking, 3 major, 3 non-blocking**.
