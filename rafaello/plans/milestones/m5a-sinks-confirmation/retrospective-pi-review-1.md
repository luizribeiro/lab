# m5a retrospective.md — pi review round 1

Reviewed `retrospective.md` at `a47a4a2` against ratified `scope.md`,
ratified `commits.md`, the live `rafaello-v0.1..HEAD` history/source, m4
retrospective precedent, `decisions.md` / `overview.md` / `glossary.md`, and
all stream RFCs.

## Blocking findings

### B1. §3/§8 miss a real c38 acceptance-test deviation

`retrospective.md` says only two plan-row deviations occurred (the c18→c38
`SlashHandler` lock cutover and the c37→c39 env allowlist follow-up), and §8
claims all positive matrix rows are tested. That is not true against the
ratified c38 row.

`commits.md` c38 acceptance names these tests:

- `rfl_chat_tool_dispatch_goes_through_gate.rs`
- `rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly.rs`
- `rfl_chat_spawns_inactive_provider_but_reemit_ignores_it.rs`
- `core_tools_list_registered_before_provider_spawn.rs`

None exist in the live tree (`find rafaello -name <name>` returns zero for all
four). The c38 commit body only lists three tests:
`agent_loop_does_not_dispatch_tool_request_directly`,
`rfl_chat_no_double_dispatch_when_gate_constructed`, and
`rfl_chat_constructs_gate_before_provider_spawn`; `git show --name-only ce0342b`
confirms the four ratified c38 acceptance files did not land.

This is exactly the sort of retrospective slippage m4's review precedent tried
to prevent: a ratified acceptance row changed during Phase 3, but §3 still says
"39 landed exactly as written" and §8 still says the positive matrix is fully
covered. Either document the c38 test-name/coverage substitution explicitly, or
add the missing tests before the retrospective can be ratified.

### B2. §2.4/§5/§8 invent an unresolved audit-SQLite location decision

The retrospective repeatedly says the audit-log SQLite path is not pinned and
routes an "audit_events SQLite on-disk location decision" to m6. The live source
has already made a concrete, stable choice:

- `rafaello/crates/rafaello/src/lib.rs:310` opens `SessionStore` at
  `<project_root>/.rafaello/state`.
- `rafaello/crates/rafaello-core/src/session/mod.rs:92` opens
  `session.sqlite` in that directory and migrates `audit_events` in the same DB.
- `AuditWriter::open_for_install` in
  `rafaello/crates/rafaello-core/src/audit/mod.rs:91-99` documents and opens
  `${project_root}/.rafaello/state/session.sqlite`.

So §2.4's concrete claims are false in two directions: c27 does not hard-code
`<project_root>/.rafaello/state/audit.sqlite`, and `rfl chat` does not inherit
an audit DB path from an in-flight tempdir. §5 item 5 and §8 manual-validation
bullet 6 should be rewritten as "document/verify the chosen
`.rafaello/state/session.sqlite` location" rather than "decide/locate TBD".

### B3. Drift/follow-up coverage omits scope-ratified non-stream documentation work

The retrospective's §6/§7 drift sweep is narrower than the ratified scope's own
post-m5a drift checklist. `scope.md` near the convergence checklist explicitly
called out, in addition to Stream F `allow_secrets`, these follow-up patches:

- Stream A §5.6 confirmation-payload/correlation clarification.
- `decisions.md` row for `core.tools_list` / `CorePluginService` (marked
  required in scope).
- `overview.md` §4.6 distinction between core-injected reserved env vars and
  well-known plugin-config env vars.
- `overview.md` §15.1 manifest shape update for `allow_secrets`.
- `glossary.md` `Audit log` entry and confirmation-protocol update.

`retrospective.md` only sketches three `decisions.md` rows and Stream A/F RFC
patches. That leaves ratified drift work untracked in the final design artifact.
It also means §7's row sketches are not anchored in the decision-table style
(`Refines/Reverses` column): `env.allow_secrets` needs an explicit refinement
anchor to the manifest/env-scrubbing line, `grant_match` should name the row it
refines/clarifies, and the m5a/m5b split should anchor to the roadmap split
language rather than being free-floating prose.

## Major findings

### M1. §2.7 falsely says c16 cached a compiled JSON-Schema validator on grants

The live c16 implementation does not cache a compiled validator on the grant
entry. `UserGrant` stores only `tool`, `plugin`, `matcher`, `added_at`, and
`source`; `UserGrants::compile_template` creates a local
`jsonschema::JSONSchema::compile(schema)` value, validates the template, then
stores `GrantMatcher::Structural { template }`.

The retrospective's sentence "caches the compiled validator on the grant entry,
then discards the validator after compile" is both internally contradictory and
not verifiable in `rafaello-core/src/user_grants.rs`. Keep the important design
claim (template validated at `/grant`; runtime is structural subset), but delete
the cache claim.

### M2. §1/§8 test-file inventory numbers do not match the requested checks

The requested coverage check gives:

```text
find rafaello -name "*.rs" -path "*tests*" | wc -l  => 605
new top-level tests vs rafaello-v0.1                  => 206
rafaello-v0.1 top-level tests                         => 400
```

The retrospective's "206 new tests" claim is correct, but the surrounding
inventory is not: it says the pre-m5a inventory was ~382 and m5a brings the
workspace to ~588. Against the actual `rafaello-v0.1` baseline and live tree,
the comparable totals are 400 and 605 (with one deleted/renamed test in the
range). Fix the totals or qualify which older m4 snapshot is being quoted.

### M3. §5 item 6 invents new production `result_large_err` suppressions

The follow-up table says m5a "likely adds two new sites (`gate/mod.rs`,
`slash.rs`)". Live grep disagrees:

```text
rg 'allow\(clippy::result_large_err\)' rafaello/crates
```

shows production allows only in `session/mod.rs`, `supervisor.rs`, `reemit/mod.rs`,
`bus.rs`, and `agent/mod.rs` — the same production set present at
`rafaello-v0.1`. m5a added one test-file allow
(`broker_plugin_tool_result_race_two_concurrent_publishes.rs`), not production
allows in gate/slash. The follow-up should not route a false m5a regression.

### M4. §8 repeats stale/manual-validation-invalid event and command names

The manual-validation skeleton section in the retrospective says to record a
`core.session.confirm_answered` event for each key. No such topic/event exists
in live code; the wire topics are `frontend.tui.confirm_answer`,
`core.session.confirm_reply`, and `core.session.confirm_resolved`, with audit
kinds such as `confirm_allowed`/`confirm_denied`.

Also, the skeleton's trifecta demo says `rfl install <fixture>`, while the live
CLI is `rfl install --fixture <PACKAGE_DIR>` (`install.rs` and `clap` args).
The retrospective should call out these skeleton corrections, not only say the
sections need output filled in.

## Non-blocking notes / polish

### N1. §6.1 points at the wrong Stream A anchor

The m4 provider/live-wire status banner is in Stream A §5, not §10. Stream A
§10 already contains the sink-confirmation/user-grants summary text. If the
follow-up patch is meant to add an m5a implementation-status banner, point it
at the §5 status banner plus any precise §5.6/§7.2 clarifications, not "§10
banner" generically.

### N2. `rfl status` marker copy is slightly off

Several retrospective passages say yellow `explicit secret <NAME>` or
`[explicit secret <NAME>]`. The ratified/live c28 copy is TTY yellow
`explicit secret: <names>` and non-TTY `[SECRET: <names>]`. Use the exact live
copy in §7.1 and §10.3.

### N3. `TestHooks` description should mention `test-fixture`

§2.3 calls the `first_spawn_instant_nanos()` seam `cfg(test)`. The live impl is
`#[cfg(any(test, feature = "test-fixture"))]`, because integration/fixture
harnesses use the seam outside unit-test cfg. Minor, but the retrospective is
trying to document reusable test-seam precedent, so the cfg should be exact.

## Confirmation table

| Check | Result |
|---|---|
| `git log --oneline rafaello-v0.1..HEAD` | 70 commits at review time; 41 plan-row commits from `f5f3062..5e36890` plus docs and the retro draft. |
| `git diff rafaello-v0.1..5e36890 --shortstat` | Matches retro's c41-tip stat: 338 files, 28,246 insertions, 254 deletions. Current HEAD includes the retro draft, so `rafaello-v0.1..HEAD` is 339 files / 28,878 insertions. |
| Stream files changed in m5a range | none; Stream A/F drift is real and must be patched as follow-up. |
| Named demo tests in retro | Present, except the intentionally deferred `rfl_chat_demo_bar_verbatim_exfil_blocked.rs`. |
| Ratified c38 acceptance test names | Four missing, see B1. |
| Audit DB path | Live path is `.rafaello/state/session.sqlite`, not `audit.sqlite` or tempdir. |
| `result_large_err` suppressions | No new production gate/slash suppressions. |

## Items checked vs items found

- **Issues raised:** 10 total — 3 blocking, 4 major, 3 non-blocking.
- **Most important fix before ratification:** reconcile c38's missing ratified
  acceptance tests and the false audit-location follow-up.
