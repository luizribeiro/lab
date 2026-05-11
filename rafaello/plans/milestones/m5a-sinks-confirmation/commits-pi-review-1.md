# m5a commits.md round-1 pi review

> Verdict: blocking.
> Counts: B/9 M/5 N/3

Reviewed `scope.md` round 6, `commits.md` round 1, prior m4/m1 commit-plan reviews, `plans/README.md`, and live source under `rafaello/crates/`. I also checked the m4 provider-refusal history: `e0fb7a9 refactor(rafaello-core::supervisor): remove row-39 refusal + wire provider broker registration` is present, and this draft does not manufacture a new removal commit.

Coverage is broadly on the ratified shape, but several rows cannot be handed to per-commit agents yet: two signature cutovers defer required call-site updates to later rows, the agent-loop pivot is ordered before `rfl chat` constructs the gate, fixture manifests fail the live package validator, and one gate row weakens `always_allow_session` beyond scope.

## Blocking findings

### B1 — c04 names an invalid `hyper = { workspace = true }` dependency and leaves the server choice to the agent

Anchor: c04 (`rafaello-openai-stub` scaffold).

The row says the stub crate should depend on:

> `hyper = { workspace = true, features = ["server", "http1"] }` … `(or httpmock if hyper workspace alias not present — pick hyper since reqwest already pulls it in transitively; no new workspace dep). Confirm at commit time whether hyper is already a workspace dep; if not, fall back to a small hand-rolled ... server`.

Live `rafaello/Cargo.toml` has no `hyper` workspace dependency; c02 only adds `reqwest` and `jsonschema`. A transitive dependency pulled by `reqwest` cannot be used as `workspace = true`. The either/or wording also violates the m1 §4.2 inline-row rule: the per-commit agent must not choose the design.

Scope §W2 only requires a deterministic localhost stub behind `test-fixture`; it does not require `hyper`.

Smallest fix: pick one concrete path. Either add `hyper` explicitly to c02's workspace dependencies and cite it in c04, or remove `hyper` from c04 and require the hand-rolled `tokio::net::TcpListener` parser. Do not leave a fallback branch in the row.

### B2 — c14 changes `ReemitRouter::new` but defers required live call-site wiring to c39

Anchor: c14 (`frontend.tui.confirm_answer` re-emit arm).

The row says:

> Extend `ReemitRouter` with an `Arc<ConfirmState>` field (new third constructor arg alongside `Arc<Broker>` / `Arc<BrokerAcl>`). ... Use the c08 `AuditWriter` ... if no such field exists today, c14 adds it.

Live source is different: `ReemitRouter::new(broker: Broker, acl: BrokerAcl, active_provider: CanonicalId, shutdown_rx: watch::Receiver<bool>)` is called from `crates/rafaello/src/lib.rs` and many `rafaello-core/tests/reemit_*` files. There is no existing audit-writer field; failures publish `core.lifecycle.reemit_rejected`, not audit rows.

c39 later says `rfl chat` constructs the shared `Arc<ConfirmState>` and passes it to `ReemitRouter`, but c14's constructor change would break the workspace immediately unless every existing call site is updated in c14.

Smallest fix: make c14 self-sufficient. Either (a) keep `ReemitRouter::new` backward-compatible and add a `with_confirm_state_and_audit(...)` builder used by c14 tests and c39, or (b) in c14 update every live call site (run_chat + reemit tests) with an inert `Arc<ConfirmState>` and c08 `AuditWriter`, and spell out the necessary run_chat reordering. Do not defer the call-site update to c39.

### B3 — c25 removes agent-loop dispatch before `rfl chat` constructs the gate

Anchor: c25 (CG6 agent-loop pivot) and c39 (CHAT1 orchestration).

c25 removes `agent/mod.rs::handle_tool_request`'s direct `broker.publish_for_tool_dispatch(...)`, but c39 is the first row that constructs and spawns `ConfirmationGate` from `rfl chat`.

Scope §CG6 requires the pivot:

> The current ... direct call to `broker.publish_for_tool_dispatch` is **removed** ... The gate is now the sole driver of the dispatch publish.

Scope §CHAT1 supplies the other half:

> construct the `ConfirmationGate` ... spawn its task ... then proceed with m4's existing supervisor + plugin spawn + agent loop construction.

As ordered, after c25 and before c39 the existing m4 `rfl chat` tests (`rfl_chat_demo_bar_read_file`, `rfl_chat_harness_finalizes_nine_entries`, etc.) have no dispatch driver. c25's acceptance mentions migrating some direct-Broker tests, but not the live `rfl chat` end-to-end tests that currently rely on agent-loop dispatch.

Smallest fix: make the pivot an unsplittable cutover: in the same commit that removes direct dispatch, wire `rfl chat` to construct a minimal gate for existing passthrough/non-sink flows. Alternatively move c25 after the c39 gate wiring and adjust dependencies so per-commit green is preserved. If kept separate, c25 must explicitly update every existing `rfl_chat_*` test expectation that would otherwise lose tool dispatch.

### B4 — c31/c35 fixture manifests will fail live `validate_with_package` without package `bin/` entry shims; c35 also lacks `openrpc.json`

Anchors: c31 (`rafaello-mailcat` manifest) and c35 (`rfl-openai` manifest).

Live `manifest::validate_with_package` checks both `openrpc.json` sibling presence and that `entry` resolves to an existing file inside the package. The validator's comment says it performs:

> `openrpc.json` sibling presence, `entry` resolution + file-vs-dir + escape, `grant_match` resolution + presence

c31 creates `entry = "bin/rfl-mailcat"` and a Rust bin at `src/bin/rfl_mailcat.rs`, but no package-relative `bin/rfl-mailcat` shim/file. c35 creates `entry = "bin/rfl-openai"`, but no `bin/rfl-openai` shim and no `openrpc.json` at all. Yet both rows include manifest/package compile-style acceptance (`mailcat_openrpc_*`, `openai_manifest_compiles.rs`). This is the same failure shape m4 review-1 B1 caught.

Smallest fix: in c31 and c35, create package fixture `bin/rfl-*` executable shims/symlinks before any `validate_with_package` acceptance. Also add an `openrpc.json` sibling for `rfl-openai` (even if it declares no tool methods) or explicitly make the acceptance use a fixture package that includes one.

### B5 — c32 changes `PluginSupervisor::new` but says the `rfl chat` call site lands in c39

Anchor: c32 (`ToolSchemaCatalog` + `PluginSupervisor` signature extension).

The row says:

> Extend `PluginSupervisor::new` ... from `(broker, config)` to `(broker, config, tool_catalog: Arc<ToolSchemaCatalog>)` ... `rfl chat` orchestration call site lands in c39.

Live source has `PluginSupervisor::new(broker, SupervisorConfig::default())` in `crates/rafaello/src/lib.rs` plus many supervisor tests. Changing the signature in c32 without updating those call sites makes the workspace fail before c39.

Smallest fix: c32 must update all live call sites in the same commit. For tests, provide an explicit `ToolSchemaCatalog::empty_for_tests()` or fixture catalog. For `run_chat`, either build the real catalog in c32 (the function already has `plugin_dirs` and `compiled_plugins`) or preserve a backward-compatible constructor and add the catalog-aware constructor in c39.

### B6 — W3's complete m5a fixture lock is never created before c40 uses it

Anchors: c35 and c40.

Scope §W3 requires:

> Workspace fixture lock under `rafaello/fixtures/m5a-locks/` containing two manifests: `rfl-openai` ... and ... `rafaello-mailcat`.

c35 only creates an openai `lock-fragment.toml` and says it is:

> merged into the demo-bar lock at c41

But c40, before c41, already says it will:

> Spawn `rfl chat` against the m5a fixture lock (`fixtures/m5a-locks/...`) with `rfl-openai` active + `rfl-mailcat` installed.

Also c40 does not depend on c35. There is no row that creates the full combined fixture lock before the headline demo test consumes it.

Smallest fix: create the complete `rafaello/fixtures/m5a-locks/rafaello.lock` (or an explicitly named complete lock file) in c35 or c40, including both openai and mailcat entries, and make c40 depend on c35. Remove the "merged at c41" text or move c40 after the merge commit.

### B7 — c22's `always_allow_session` grant path conflicts with scope and can broaden a grant to `Any`

Anchor: c22 (CG4 allow + grant creation).

Scope §CG4 says the gate creates the session grant as:

> `UserGrant { plugin: held.dispatch_target, tool, matcher: Structural::from_args(args), source: AlwaysAllowSession }`

c22 instead says to call `UserGrants::compile_template(&tool, args, tool_grant_match_schemas.get(&tool))?`, and then adds:

> If `compile_template` errors, fall back to `GrantMatcher::Any` and audit a warning; the grant still authorises future identical calls. ... JSON-Schema validation is skipped at gate time ...

This is both contradictory and broader than scope. Falling back to `Any` after an args/schema mismatch can authorize future calls that do not match the args the user just approved.

Smallest fix: remove schema validation and the `Any` fallback from the CG4 `always_allow_session` path. Construct exactly `GrantMatcher::Structural { template: args.clone() }` / `Structural::from_args(args)` as scope requires, and drop c22's dependency on c16 if it only existed for `compile_template`.

### B8 — M1.1's explicit no-new-reserved-env coverage is missing

Anchor: c06/c35 self-audit mapping of M1.1.

Scope §M1.1 is not just "allow_secrets in disguise". It explicitly requires:

> No new reserved env-var names in m5a ... Tests:
> - `compile_openai_lock_with_rfl_openai_envset_keys_succeeds.rs`
> - `scrubber_reject_reserved_unchanged_for_seven_core_names.rs`

commits.md self-audit maps `M1.1 → c06 (allow_secrets cutover is "no new reserved names" in disguise)`, but c06 does not name either test. c35 has related openai env tests, but not the explicit RFL_OPENAI env-set compile test or the seven-core-name regression.

Smallest fix: add `scrubber_reject_reserved_unchanged_for_seven_core_names.rs` to c06, and add/rename the openai lock compile test in c35 to the exact scoped behaviour (`compile_openai_lock_with_rfl_openai_envset_keys_succeeds.rs`, or a clearly named equivalent in the trace table).

### B9 — c31/c32's claimed two-stage OpenRPC tests are not a valid two-stage ladder

Anchor: c31 and c32.

The README's m0 §4.3 rule says two-stage tests should exercise whatever surface exists in the earlier commit, then extend it when the later surface lands; do not punt the whole test to the last commit.

c31 acceptance names:

> `mailcat_openrpc_method_matches_provides_tools.rs` ... `ToolSchemaCatalog::build` succeeds ... lands at c32 once `ToolSchemaCatalog` exists; this row's test file stages an assertion against `manifest::validate_with_package` for now
>
> `mailcat_openrpc_missing_method_for_provides_tools_errors.rs` ... assertion staged at c32.

But live `validate_with_package` only checks `openrpc.json` sibling presence, not method-vs-tool consistency. A typo'd method fixture cannot error until c32's `ToolSchemaCatalog` exists. The c31 negative test is therefore either unimplementable, ignored, or misleadingly green against the wrong behaviour.

Smallest fix: c31 should only test the surfaces that exist there (`manifest::validate_with_package` succeeds with an `openrpc.json` sibling and `grant_match` path). Move the method-match positive and missing-method negative fully to c32, with c32 explicitly extending the c31 fixtures.

## Major findings

### M1 — c24/c27 have no TUI-visible signal for grant short-circuited queued confirmations

Anchors: c24 (CG7 short-circuit) and c27 (TUI queue pruning).

Scope §CG7 says queued prompts are pruned when:

> the TUI's overlay, on observing that a held entry has been resolved server-side, drops the corresponding queued prompt ... (`core.session.confirm_reply` events arrive on the TUI's bus subscription and the TUI tracks held→reply correlation for queue-pruning).

c24 resolves and dispatches matching held entries directly, but it does not publish any `core.session.confirm_reply` (nor any other bus event carrying the short-circuited confirm id). c27 then says the TUI drops queued entries on `core.session.confirm_reply`. As written, queued prompts short-circuited by a new grant can remain in the TUI queue until the operator answers a stale modal.

Smallest fix: add an explicit bus-visible resolution signal for each short-circuited confirm id and specify how the gate avoids re-consuming its own signal. If the intended signal is not `confirm_reply`, update both scope mapping and c27 to name the real event the TUI observes.

### M2 — c11 under-tests the new exact-one `in_reply_to` rule

Anchor: c11.

Scope §CT3 and §SL0 require exact-one `in_reply_to` for `frontend.tui.confirm_answer`, `core.session.confirm_reply`, and `core.session.command_result`. c11 tests missing and too-many for `confirm_answer`, but only missing for `confirm_reply` and `command_result`.

Smallest fix: add `broker_publish_core_session_confirm_reply_in_reply_to_too_many_rejected.rs` and `broker_publish_core_session_command_result_in_reply_to_too_many_rejected.rs` (or one clearly named parameterised test that covers both exact-one cases).

### M3 — c28 audits install rows without specifying how `rfl install` opens the audit database

Anchor: c28.

Scope §AL2 wires `AuditWriter` into `rfl install`, and c28 step 12 audits `install_accepted` / `trifecta_overridden`. But c08's `AuditWriter` is handed out by `SessionController::audit_writer(&self)`, while `rfl install` does not run a chat session or construct a `SessionController`.

Smallest fix: c28 should state the concrete install-side construction path: open the same `${PROJECT_ROOT}/.rafaello/state/session.sqlite` store (respecting the session lock semantics or explicitly bypassing chat-only locking), run the audit migration, and create `AuditWriter`. Otherwise agents will invent incompatible audit locations.

### M4 — c32's dependency list omits c31 despite extending c31 tests/fixtures

Anchor: c32.

c32 says it "extends c31 two-stage tests" for mailcat OpenRPC, but its dependency line is only `c01`. Phase order happens to put c31 before c32, but per-row dependency metadata should be complete for the driver and for cherry-pick recovery.

Smallest fix: add `c31` to c32's `Depends on` once the two-stage test shape is corrected.

### M5 — c41's status bonus depends on c29 but the row does not say so

Anchor: c41.

c41 includes `rfl_install_status_shows_red_for_override.rs`, which exercises the `rfl status` subcommand from c29. Its dependency list is `c30, c31, c39, c40`; c30 depends on c28, not c29.

Smallest fix: add c29 to c41 dependencies, or move that bonus test into c29's acceptance where the status surface lands.

## Nit findings

### N1 — Header and sizing summary disagree on total/size counts

The header says "Total: 41 commits" while the plan has c01-c42 and later says "Total: 42 commits." The sizing summary says "medium: 7 commits" but lists eleven commit ids. Fix the arithmetic before round 2 so reviewers can use the counts as a drift signal.

### N2 — Several row-local references have stale commit numbers/phase names

Examples: c14 says CG4 grant creation happens in "c19" even though CG4 is c22; c15's Why says the slash handler is "Phase G c25"; Phase H prose refers to gate as Phase G in one spot. These are small, but per-commit prompts are inlined; stale numbers create unnecessary agent confusion.

### N3 — W4 is not assigned a concrete home

Scope §W4 says the workspace README (if any) gains a one-line "m5a adds rfl-openai" metadata note. The self-audit maps W1-W4 to c01-c05, but no row mentions W4. If there is no workspace README, say so explicitly in c03 or c42; otherwise add the one-line doc edit to a scaffolding/docs row.

## Convergence call

Round 1 should not converge. I expect at least one more blocking round after the author fixes the cutover ordering (c14/c25/c32), the fixture-validator issues, and the missing W3/M1.1 coverage. Owner-judgment items from scope still stand (`m5a`/`m5b` split, grant matcher interpretation, `env.allow_secrets`), but none of this review requires changing those choices; the fixes are commit-plan mechanics and scope-faithfulness.
