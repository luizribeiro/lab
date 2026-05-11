# m5a retrospective.md — pi review round 3

Reviewed round-3 `retrospective.md` at `3334a5e`, the diff from `18abc75`, and the live source/tests named by the round-2 findings.

## Blocking findings

### B1. §8 / §6.1 still overstate `core.session.confirm_resolved` emission

Round 3 fixed the TUI key mapping, but the manual-validation and Stream-A patch prose still describe a live topic that is not emitted for the normal allow/deny paths.

Live source:

- `rafaello-tui/src/lib.rs` maps `y` / `a` / `Enter` to `allow`, `n` / `d` / `Esc` to `deny`, `s` to `always_allow_session` — round 3 now gets this right.
- `reemit/mod.rs` canonicalises `frontend.tui.confirm_answer` to `core.session.confirm_reply`.
- `gate/mod.rs` audits `confirm_allowed` / `confirm_denied` and dispatches/denies the held request. It does **not** publish `core.session.confirm_resolved` for the active allow/deny confirmation.
- `core.session.confirm_resolved` is published by `short_circuit_pending_after_grant` for grant-short-circuited *other pending* confirmations (`reason: "grant_short_circuit"`). The timeout path audits `confirm_timeout` and emits a deny-shaped `tool_result`; it also does not publish `confirm_resolved`.

So §8 bullet 5 is still wrong when it tells the manual validator that "the gate publishes `core.session.confirm_resolved`" for each documented key. §6.1 repeats the same false patch shape by saying `confirm_resolved` is the gate's resolution-visibility publish on "allow / deny / short-circuit / timeout". This will make manual validation look for a topic that normal `y`/`a`/`Enter`/`n`/`d`/`Esc` runs do not produce.

Fix: describe `confirm_resolved` only for grant-short-circuit queue pruning (and any actually-live future cases), while normal answers are `frontend.tui.confirm_answer` → `core.session.confirm_reply` → gate dispatch/audit.

## Major findings

### M1. c38 gap accounting is internally inconsistent after the item-15 addition

Round 3 correctly downgrades the first c38 substitute to "partial substitute" and adds §5 item 15 for the missing positive gate-through assertion. But two nearby summaries still carry the round-2 wording:

- §3's c38 deviation table says "one substitute landed as a functional equivalent".
- §8's §CHAT caveat says "The missing regression-anchors are §5 items 12-14", omitting item 15.
- The paragraph after the follow-up table says "items 12-14 are the c38 acceptance-test deviation", also omitting item 15.

These should all say partial substitute and include §5 item 15, otherwise the coverage report still undercounts the c38 acceptance gap.

### M2. §6.4's audit-kind glossary patch is incomplete while claiming to name m5a-produced kinds

The glossary patch sketch says it names "the audit kinds produced in m5a" but omits several live `AuditKind::as_str()` values from `audit/mod.rs`: `gate_passthrough`, `gate_grant_match`, `gate_grant_match_short_circuit`, `confirm_malformed`, `confirm_resolved_after_timeout`, `grant_revoked`, `grant_list`, and `credential_paths_overridden` is included but not the related gate/slash kinds.

Either make the glossary entry exhaustive against `AuditKind`, or reword it as examples/key families rather than "the audit kinds produced in m5a".

### M3. §7.1 still says the unused-`allow_secrets` install warning is yellow

Live `install.rs` prints a plain stderr line:

```text
warning: unused allow_secrets entry '<name>' (no matching env.pass entry)
```

There is no ANSI/yellow styling on that warning. The yellow marker is the `rfl status` TTY suffix for accepted `allow_secrets` entries. Drop "yellow" from the install-warning sentence in the decision-row sketch.

## Non-blocking notes / polish

### N1. Round/status hash is still a placeholder

The round-3 status line still says `at hash TBD-round-3` even though this review is against `3334a5e`. If the retrospective keeps per-round hashes in the banner, fill it before ratification.

### N2. §2.3's monotonic-clock implementation description is imprecise

The live helper is `supervisor::monotonic_nanos()` with a `OnceLock<Instant>` epoch and `saturating_duration_since`, not literally `Instant::now().elapsed().as_nanos()`. Minor, but this section is documenting a reusable test-seam pattern, so naming the helper/epoch shape would be more accurate.

## Round-2 closure check

| Round-2 finding | Status |
|---|---|
| B1 decision anchors | Fixed. Both rows now avoid false prior decision-row anchors. |
| B2 TUI key mapping / timeout answer enum | Mostly fixed for key mapping and answer enum; new/remaining false `confirm_resolved` emission claim is blocking (B1). |
| M1 c38 partial substitute | Partially fixed; detailed row is fixed, but summaries still say/act like functional-equivalent or omit item 15 (M1). |
| M2 `install_override` | Fixed for the invented kind; see M2 for incomplete audit-kind list. |
| M3 Stream A names | Partially fixed; accessors and re-emit-vs-gate ownership corrected, but `confirm_resolved` paths are over-broad (B1). |
| N1 boilerplate | Partially fixed; footer updated, hash still TBD (N1). |
| N2 overview §4.6 | Fixed. |
| N3 shortstat command | Fixed. |

## Verdict

Not ready for owner ratification yet. Issues raised: **1 blocking, 3 major, 2 non-blocking**.
