# m5a commits.md round-3 pi review

> Verdict: blocking, but narrow.
> Counts: B/1 M/3 N/1

Reviewed the round-3 `commits.md` at `dc8da82`, the round-2 review, the `e054ee1..HEAD` diff, ratified `scope.md`, and live source under `rafaello/crates/`. Round 3 closes the main round-2 structural problems: c04's feature syntax is fixed, readfile OpenRPC is backfilled, the fixture lock is now four-plugin, `confirm_resolved` has a contract table, c14 has a transitional-drop test, and c38 depends on the slash handler.

One live-schema blocker remains in the canonical fixture lock text: the plan still uses `session.active_provider`, while the live lock schema field is `provider_active`.

## Findings

### B1 — c34 still uses the wrong live lock field: `session.active_provider` instead of `session.provider_active`

Anchor: c34 (`rfl-openai` manifest + combined fixture lock).

c34 says the complete lock sets:

> `session.active_provider = "builtin:openai@0.0.0"`

and its acceptance repeats:

> assert `session.active_provider`, `session.tool_owner.send-mail`, and `session.tool_owner.read-file` are pinned

Live source uses `provider_active`, not `active_provider`:

> `crates/rafaello-core/src/lock/session.rs`: `pub provider_active: Option<String>` with `#[serde(deny_unknown_fields)]` on `SessionTable`.

So a TOML lock containing `active_provider` under `[session]` is rejected as an unknown field, and c34's `m5a_fixture_lock_validates_and_compiles.rs` cannot pass as written. This is also a known m4/m5 terminology trap (`provider_active` vs provider id/canonical was corrected in prior rounds).

Smallest fix: replace every c34 occurrence of `session.active_provider` with `session.provider_active`, and rename the acceptance assertion to `m5a_fixture_lock_session_pins_provider_active_and_tool_owners.rs` (or equivalent) so the file name teaches the live field.

## Major findings

### M1 — c11's positive `confirm_resolved` wire-shape test is placed before the gate publisher exists

Anchor: c11.

Round 3 adds the needed `confirm_resolved` table, but the positive test in c11 says:

> `broker_publish_core_session_confirm_resolved_wire_shape_positive.rs` ... gate publishes a well-formed `confirm_resolved` event

The gate publisher is introduced in c24. c11 only adds topic constants and broker suffix validation. If the test is meant to be a broker-level publish, it should not say the gate publishes it; if it is meant to exercise the real gate publisher, it belongs in c24 alongside `gate_grant_short_circuit_publishes_confirm_resolved.rs`.

Smallest fix: either reword the c11 positive as a direct `Broker::publish_core_with_taint`/internal-subscriber wire-shape test, or move the real gate-publisher positive to c24 and keep c11 to broker negative/cardinality coverage.

### M2 — c38's five-tree cutover needs an explicit live spawn-loop edit for inactive providers

Anchor: c38.

Scope §CHAT2 says the m5a fixture keeps readfile and mockprovider as installed-but-not-active alternatives and that:

> Every plugin spawned through the existing `PluginSupervisor`.

Round 3's c34 lock now includes mockprovider, and c38 accepts `rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly.rs`. But live `run_chat` currently spawns the active provider, then iterates lock entries and skips entries where `entry.bindings.provider` is true; an installed-but-not-active provider like mockprovider is therefore not spawned by the live loop.

c38 says the tree becomes five-tree, but it does not explicitly instruct the agent to change that provider-skipping loop to spawn installed inactive providers too. The acceptance will catch it late, but the row should be self-contained.

Smallest fix: add a concrete c38 bullet: after spawning the active provider, also spawn every other installed provider entry through `PluginSupervisor` as inactive/non-reemitted providers, then spawn tool plugins. Keep `ReemitRouter` subscribed only to `session.provider_active`.

### M3 — c34's combined-lock test should also build the `ToolSchemaCatalog`

Anchor: c34.

c34 acceptance currently says the combined four-plugin lock:

> passes `validate::lock` and `compile_plugin` for **all four** entries

That catches lock and compile shape, but the round-2 failure was specifically about `ToolSchemaCatalog::build` at `rfl chat` startup. c31 tests readfile and mockprovider fixtures separately, but c34 is the first row with the canonical m5a lock and all package-dir mappings together.

Smallest fix: extend `m5a_fixture_lock_validates_and_compiles.rs` to also call `ToolSchemaCatalog::build(&acl, &compiled_plugins, &package_dirs)` for the combined fixture lock and assert the catalog contains exactly the expected tool schemas (`send-mail` and `read-file`, not openai/mockprovider provider entries).

## Nit findings

### N1 — A few stale `c37` references remain after the numbering pass

Round 3 fixed most numbering drift, but `rg "c37" commits.md` still finds stale references in row bodies:

- c14 says the orchestration builder call lands in `c37`; it is c38.
- c31 says remaining rfl-chat plumbing lands at `c37`; it is c38.
- c34 says runtime spawn points at the cargo-built bin in `c37`; it is c38.

Smallest fix: replace those stale c37 references with c38 (or with "the rfl chat orchestration cutover" where possible).

## Round-2 verification table

| r2 finding | status | verification |
|---|---|---|
| B1 c04 invalid feature stanza | closed | c04 now uses `[features] test-fixture = []` and `required-features = ["test-fixture"]`; no mandatory dependency is listed as a feature. |
| B2 `ToolSchemaCatalog` breaks readfile fixture | closed | c31 backfills `rafaello/fixtures/rafaello-readfile/openrpc.json` with a `read-file` method and adds readfile/mockprovider catalog regression tests. The OpenRPC param `path: string` matches live `rfl_readfile.rs`, which reads `payload.args.path`. |
| B3 two-plugin fixture lock vs five-tree | closed for original | c34 now requires a four-plugin lock (openai + mailcat + readfile + mockprovider) and c34 depends on c31. New live-field blocker tracked as B1. |
| B4 stale answer after short-circuit audit kind | closed | c24 now expects `confirm_duplicate`, matching `ResolvedByAnswer -> Duplicate` in c13. |
| M1 `confirm_resolved` wire contract | mostly closed | c11 adds a table and positive wire-shape acceptance; placement/wording of the positive test needs cleanup (M1). |
| M2 install audit parent dir | closed | c27 now calls `create_dir_all(.rafaello/state)` and adds a fresh-tempdir audit test. |
| M3 c38 missing c18 dependency | closed | c38 depends on c16 and c18. |
| M4 mailcat `grant_match` path | closed | c34 pins `bindings.tool_meta.send-mail.grant_match = "schemas/send-mail-grant.json"`, explicitly not OpenRPC. |
| M5 c14 transitional drop test | closed | c14 adds `reemit_confirm_answer_without_confirm_state_warns_and_drops.rs`. |
| N1-N3 numbering/cross-check drift | mostly closed | Cross-check and sizing are now coherent; a few stale row-body `c37` references remain (N1). |

## Convergence call

Not clean yet: there is **one blocker**. The fix is small and mechanical (`provider_active`), plus a few row-tightening edits around `confirm_resolved`, five-tree spawn wording, and stale numbering. After those changes, I would expect round 4 to be a zero-blocker convergence pass if no new surface is introduced.
