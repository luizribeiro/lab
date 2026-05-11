# m5a commits.md round-2 pi review

> Verdict: blocking.
> Counts: B/4 M/5 N/3

Reviewed the round-2 `commits.md` at `e054ee1`, the round-1 review, the `6dc0da3..HEAD` diff, ratified `scope.md`, and live source under `rafaello/crates/`. Round 2 closes most of the round-1 mechanical gaps, but new/changed surfaces still leave the plan non-ratifiable: c04's feature stanza is not valid Cargo, the `ToolSchemaCatalog` cutover would break existing `rfl chat` fixtures, the combined m5a lock contradicts the five-tree scope, and the new `confirm_resolved` short-circuit path has a mechanically wrong stale-answer expectation.

## Findings

### B1 — c04's `test-fixture` feature lists a non-optional dependency as a feature

Anchor: c04.

Round 2 correctly removes the invalid `hyper = { workspace = true }` fallback, but the replacement still cannot compile as written:

> `test-fixture` feature that depends on `tokio/macros`, `tokio/rt-multi-thread`, `serde_json`.

In Cargo, a feature entry may reference another feature (`tokio/macros`) or an optional dependency (`dep:serde_json` / `serde_json` if optional). c04 also declares `serde_json` as a normal dependency, not optional. That produces the usual Cargo error that the feature includes `serde_json` but it is not an optional dependency.

Live precedent: `rafaello-core`'s `rfl-bus-fixture` uses:

> `[features] test-fixture = []` and `required-features = ["test-fixture"]`.

Smallest fix: make `test-fixture = []` (or only `tokio/...` feature toggles if those dependencies are truly optional). Do not list mandatory `serde_json` in the feature. If the goal is to avoid building the bin by default, `required-features = ["test-fixture"]` is sufficient.

### B2 — c31 wires `ToolSchemaCatalog::build` into `run_chat` before existing tool fixtures have matching OpenRPC methods

Anchor: c31.

c31 now updates the live `PluginSupervisor::new` call site in `run_chat` and says:

> build the real catalog via `ToolSchemaCatalog::build(&acl, &compiled_plugins, &package_dirs)` ... and pass `Arc::new(catalog)` as the third arg.

But `ToolSchemaCatalog::build` is also specified to fail when any `provides.tools` entry has no matching OpenRPC method:

> fail with `ToolMissingOpenRpcMethod` ... live `validate_with_package` only checks sibling presence ... NOT this consistency.

Live `rafaello/fixtures/rafaello-readfile/rafaello.toml` declares `tools = ["read-file"]`, while live `rafaello/fixtures/rafaello-readfile/openrpc.json` has `"methods": []`. Existing m4 `rfl_chat_*` tests use the readfile fixture. As soon as c31 wires the real catalog into `run_chat`, those tests fail before any m5a openai/mailcat work is involved.

Smallest fix: in c31, update every existing in-tree tool fixture that can appear in a `run_chat` lock (at least `rafaello-readfile`) with matching OpenRPC methods, or scope `ToolSchemaCatalog::build` to only the active provider-visible tool set and spell that out. Add a regression acceptance item that the existing m4 readfile chat fixture still starts after the catalog cutover.

### B3 — c34's "complete" fixture lock contradicts scope §CHAT2 and c38's five-tree acceptance

Anchors: c34 and c38.

Scope §CHAT2 requires the m5a orchestration tree to become:

> `rfl chat` → `rfl-tui` + `rfl-openai` + `rfl-mailcat` (+ `rfl-readfile` and `rfl-mockprovider` retained as installed-but-not-active alternatives in the same fixture lock for the negatives).

c38 repeats that five-tree claim and accepts:

> `rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly.rs` — extend m4's ... smoke test to assert all five children reap on shutdown.

But c34's combined lock says it contains entries for **both** `builtin:openai@0.0.0` and `local:mailcat@0.0.0`, and its acceptance only compiles "both entries". There is no row that adds the retained readfile/mockprovider entries to the canonical lock. With only two plugins, c38's five-tree smoke cannot pass; with four plugins, c34/c31 must also handle their OpenRPC/catalog data.

Smallest fix: either (preferred, matching scope) make c34's `rafaello/fixtures/m5a-locks/rafaello.lock` include openai + mailcat + readfile + mockprovider and update the c34 acceptance accordingly, or explicitly change c38 away from five-tree semantics and surface that as a scope change. Do not leave c34 and c38 describing different canonical locks.

### B4 — c24 expects `confirm_late` after short-circuit, but `ConfirmState` will classify it as duplicate

Anchor: c24.

c24 short-circuits a held entry by calling:

> `state.try_resolve(entry_id)`

c13 defines `try_resolve` as `Active → ResolvedByAnswer`, and `prior_outcome` maps:

> `ResolvedByAnswer` → `Duplicate`; `TimedOut` → `Late`.

Yet c24 acceptance says a stale answer after short-circuit audits `confirm_late`:

> `prior_outcome` read returning `Late` because the short-circuit set `ResolvedByAnswer`.

That is mechanically false against the same plan's `ConfirmState` table. Implementing c13 literally makes the c24 test expect the wrong audit kind.

Smallest fix: change the c24 stale-answer expectation to `confirm_duplicate` (short-circuit is a non-timeout resolution), or add a separate `ShortCircuited` tombstone and update c13/CT5/AL tests accordingly. The smaller fix is the duplicate audit.

## Major findings

### M1 — the new `core.session.confirm_resolved` bus surface lacks a table-of-truth-level wire contract

Anchors: c11, c24, c26.

`confirm_resolved` is a new core topic introduced in round 2. c11 adds suffix rules and negative broker tests; c24 says the gate publishes payload `{request_id, reason}` and `in_reply_to = [confirm_id]`; c26 says the TUI drops by `payload.request_id`.

Missing pieces compared to the rigor of scope §CT0 / §SL0:

- whether the envelope `request_id` is a fresh event id or equals the confirm id;
- c24 does not explicitly say to pass an envelope `request_id` at all, even though c11 adds `"confirm_resolved"` to `REQUEST_ID_REQUIRED_SUFFIXES`;
- there is no positive broker/wire-shape test asserting the accepted event's envelope `request_id`, `payload.request_id`, and `in_reply_to` relationship.

Smallest fix: add a tiny `confirm_resolved` table in c11 (or inline in c24) with the same columns as CT0: envelope `request_id`, payload `request_id`, `in_reply_to`, stale/duplicate/late meaning. Then add/extend `gate_grant_short_circuit_publishes_confirm_resolved.rs` to assert the envelope request id is present and has the chosen semantics.

### M2 — c27's install audit constructor omits parent-directory creation

Anchor: c27.

c27 adds:

> `AuditWriter::open_for_install(project_root)` ... opens `${PROJECT_ROOT}/.rafaello/state/session.sqlite` directly via `rusqlite::Connection::open(...)`.

For a fresh project, `${PROJECT_ROOT}/.rafaello/state/` may not exist before `rfl install`; `Connection::open` does not create parent directories. Scope §Tr1's happy path is an install into a lock path, not necessarily after `rfl chat` has created session state.

Smallest fix: require `std::fs::create_dir_all(project_root.join(".rafaello/state"))` before opening the SQLite file, and add an acceptance assertion that `rfl_install_fixture_writes_lock.rs` records an audit row in a fresh tempdir with no pre-existing `.rafaello/state`.

### M3 — c38 uses the slash handler but omits c18 from its dependency list

Anchor: c38.

c38 says `run_chat` will:

> Register the core-side slash handler (c18) as an internal subscriber on `frontend.tui.slash_command`.

But c38 depends on `c08, c13, c14, c15, c20, c21, c22, c23, c24, c31, c36`; c18 is absent. Sequential phase order hides this, but the dependency metadata is false and can break cherry-pick recovery / per-row prompt context.

Smallest fix: add c18 (and therefore c16 transitively via c18) to c38's `Depends on` list.

### M4 — c34 says the mailcat lock's `grant_match` points at the OpenRPC sibling

Anchor: c34.

c34 describes the mailcat entry in the combined lock as:

> `grant_match` schema path pointing at the openrpc-sibling location.

Scope §TP2 pins the grant matcher schema at `crates/rafaello-mailcat/schemas/send-mail-grant.json`. OpenRPC is for `ToolSchemaCatalog` / model tool parameters; `grant_match` is the user-grant template schema. Pointing `grant_match` at `openrpc.json` would make slash `/grant` schema validation use the wrong schema.

Smallest fix: change the c34 wording (and fixture lock requirements) to `grant_match = "schemas/send-mail-grant.json"` / the corresponding lock `bindings.tool_meta.send-mail.grant_match` SafePath. Keep OpenRPC only for `core.tools_list` schema derivation.

### M5 — c14's unwired `confirm_answer` arm silently drops events without a regression test

Anchor: c14.

The backward-compatible builder closes the round-1 call-site blocker, but c14 now says that if `confirm_state` or `audit` is `None`, the new `confirm_answer` arm:

> drops the event silently with a tracing warning.

That is acceptable only as a transitional guard between c14 and c38, but it is a new behaviour worth pinning: existing m4-shaped `run_chat` will have c11's broker acceptance for `frontend.tui.confirm_answer` once c12 lands, yet c14 will drop it if the builder is absent.

Smallest fix: add one c14 acceptance test such as `reemit_confirm_answer_without_confirm_state_warns_and_drops.rs` asserting no `confirm_reply` is emitted and no panic occurs. That prevents the transitional path from becoming an untested black hole.

## Nit findings

### N1 — Round-2 fix-summary commit numbers are stale after renumbering

The top fix summary repeatedly names old numbers: B-4 says c29/c33 for mailcat/openai (actual c30/c34), B-5 says c30 for `PluginSupervisor` (actual c31), B-6 says c33 for the combined lock (actual c34), M-3 says c26 for install (actual c27), and M-5 says the status test moved to c27 (actual c28). These are in the preamble, not row bodies, but they are exactly the text reviewers scan first.

### N2 — Cross-check and sizing summary are still arithmetically wrong

Examples: cross-check maps `Tr1-Tr4 → c26-c28` (actual c27-c29), `TUI1-TUI5 → c25-c25` (misses c26), `CHAT1-CHAT3 → c36 + c37` (actual c37 + c38), and the unsplittable list says c37 instead of c38. The sizing summary says "small: 30" and "medium: 8" but each parenthesized list contains a different count. This is a continuation of pi-1 N1/N2.

### N3 — Header still cites c26 for the unused-`allow_secrets` string

The top paragraph says c26 pins the canonical `rfl install` warning string, but after renumbering the install row is c27. Small, but it reinforces the numbering drift.

## Round-1 verification table

| r1 finding | status | verification |
|---|---|---|
| B1 hyper fallback | closed for original; new B1 | c04 chooses a hand-rolled `TcpListener` server and removes hyper, but its feature list now names mandatory `serde_json`. |
| B2 `ReemitRouter::new` cutover | closed | c14 keeps `new(...)` backward-compatible and adds `with_confirm_state_and_audit(...)`; c38 uses the builder. |
| B3 agent-loop pivot ordering | closed for original | The pivot is bundled into c38 with gate construction and no-double-dispatch acceptance. |
| B4 fixture `bin/` shims / openrpc | closed for original | c30/c34 add `bin/rfl-*` shims and c34 adds openai `openrpc.json`; new fixture-lock/five-tree issue tracked as B3. |
| B5 `PluginSupervisor::new` call sites | partly open | c31 updates call sites, but wiring real catalog into `run_chat` exposes existing readfile OpenRPC drift (B2). |
| B6 complete m5a fixture lock | partly open | c34 creates a combined lock before c39, but it conflicts with c38/scope five-tree expectations (B3). |
| B7 `always_allow_session` Structural matcher | closed | c22 now constructs `GrantMatcher::Structural { template: args.clone() }` and drops c16. |
| B8 M1.1 tests | closed | c06 adds the seven-reserved-names test; c34 adds `compile_openai_lock_with_rfl_openai_envset_keys_succeeds.rs`. |
| B9 two-stage OpenRPC ladder | closed | c30 limits itself to package validation; method-vs-tool tests move to c31. |
| M1 short-circuit visibility | partly open | `confirm_resolved` gives the TUI a signal, but its wire contract is underspecified (M1). |
| M2 exact-one `in_reply_to` tests | closed | c11 adds too-many tests for `confirm_reply` and `command_result`. |
| M3 install audit construction | partly open | c27 adds `open_for_install`, but needs parent-directory creation for fresh projects (M2). |
| M4 c32/c31 dependency | closed | c31 depends on c30 after renumbering. |
| M5 status bonus dependency | closed | status-red test moved into c28. |
| N1/N2 numbering/sizing | still open | Row bodies improved, but preamble/cross-check/sizing still contain stale numbers (N1-N3). |
| N3 W4 home | closed | c03 explicitly appends the `rafaello/README.md` crate-list note. |

## Convergence call

Round 2 is still blocking. The main remaining work is not a broad redesign: fix c04's Cargo feature syntax, decide/repair the canonical fixture set (including existing readfile OpenRPC if the five-tree lock is retained), correct the short-circuit stale-answer audit, and tighten the new `confirm_resolved` wire contract. After those, I expect one more review round focused mostly on numbering/trace-table hygiene and the c38 cutover details.
