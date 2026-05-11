# m5a scope.md round-5 pi review

> Verdict: non-blocking, not yet ratification-ready
>
> Counts: B/0 M/3 N/5

Round 5 resolves the round-4 blocker: `always_allow_session` no longer requires re-emit to own `UserGrants` or audit handles. The new `session_grant_requested` flag keeps held-entry ownership with the gate and preserves the public Stream A `confirm_reply` schema. The `ToolSchemaCatalog`, OpenRPC fixture, `allow_secrets` merge, and audit-kind fixes are also materially improved.

I found no blockers. The remaining issues are precision/edge-case items in newly added round-5 wording. I would do one short polish fold before owner ratification, mainly to specify the `mark_session_grant_requested` race behaviour and to place the unused-`allow_secrets` installer warning in the actual §Tr1 algorithm.

## Findings

## Blockers

None.

## Major

### M-1. `mark_session_grant_requested` error/race handling is not specified

**Anchor:** §CT5 step 5, §CG1a method table, §CG4 race note.

Round 5 adds the right primitive, but CT5 does not say what re-emit does if `mark_session_grant_requested(confirm_id)` returns `MarkError::NotActive`. That can happen even after `prior_outcome == Held`: the timeout task can win between CT5 step 4 and step 5, or another path can resolve first. If re-emit ignores the error and still publishes `core.session.confirm_reply { answer: "allow" }`, the gate will later drop it via `try_resolve == None`, but CT0's late-answer contract says no confirm_reply is emitted for late answers.

**Smallest fix:** In CT5 step 5, handle `MarkError::NotActive` explicitly: re-read/classify `prior_outcome`, audit `confirm_late` / `confirm_duplicate` / `confirm_unknown` as appropriate, and **do not** publish `core.session.confirm_reply`. Add `reemit_confirm_answer_always_allow_session_races_timeout_drops_without_reply.rs` (or equivalent).

### M-2. The unused-`allow_secrets` warning is not integrated into the install algorithm

**Anchor:** §OP6 validation rules, §Tr1, §Tr4.

§OP6 now correctly moves unused-name warnings out of `validate::lock` and into `rfl install` stderr, with audit details. But §Tr1's mechanical install algorithm still goes directly from `validate::lock` to optional trifecta diagnostics to writing the lock/audit row; it never computes or emits `unused_allow_secrets`, and §Tr4 does not list the new warning test.

**Smallest fix:** Add a Tr1 step after successful validation and before writing the lock: compute unused `allow_secrets` entries for the candidate lock entry, print one stderr line per unused name, and include them in the `install_accepted` audit payload. Add `rfl_install_warns_on_unused_allow_secrets_entry.rs` to §Tr4 as well as §OP6.

### M-3. `compile.rs::effective_grant` is described as default+named, but live spawn env merges all bundles

**Anchor:** §OP6 effective-grant merge; live `compile.rs:260-312`.

Round 5 moved `allow_secrets` to the correct function, but says `compile.rs::effective_grant` unions env across "the `default` bundle and the named bundle." Live `compile.rs::effective_grant` iterates `for bundle in grant.bundles.values()` and unions **all** bundles into the spawn-time policy. If the m5a edit followed the text literally, `allow_secrets` would diverge from `env.pass`/`env.set` and ignore some bundles that the current spawn env includes.

**Smallest fix:** Reword §OP6 to say `compile.rs::effective_grant` unions/dedups `env.pass` and `env.allow_secrets` across every `grant.bundles` value (matching the live spawn-time policy). Keep the `sinks::effective_grant(default ∪ tool)` distinction separate.

## Nits

### N-1. CT5 introduction still says the special arm "adds a UserGrant"

**Anchor:** §CT5 introduction before the validation steps.

The paragraph says re-emit's special `always_allow_session` arm "adds a `UserGrant`". Step 5 and CG4 now correctly say the gate creates the grant. Update the intro to "marks `session_grant_requested`".

### N-2. Re-emit test name still says it creates the grant

**Anchor:** §CT5 tests.

`reemit_confirm_answer_always_allow_session_creates_grant_and_emits_allow.rs` should be renamed to reflect the new ownership, e.g. `reemit_confirm_answer_always_allow_session_marks_state_and_emits_allow.rs`.

### N-3. CT0 implication 6 still says `resolve` consumed the entry

**Anchor:** §CT0 implications.

Round 5 fixed most stale method names, but implication 6 still says the first answer's `resolve` consumed it. Use `try_resolve`.

### N-4. OP6 manifest `EnvCapabilities` snippet omits the live `Serialize` derive

**Anchor:** §OP6 manifest schema snippet; live `manifest/capabilities.rs`.

The live type derives both `Deserialize` and `Serialize`. The snippet should include `Serialize` to avoid a needless diff from the existing shape.

### N-5. OP5 still says `allow_secrets` is snapshotted "into bindings"

**Anchor:** §OP5 lock snippet comments.

Round 5 correctly moved the lock location to `grant.bundles.<bundle>.env.allow_secrets`, but OP5's comment still says the lock snapshots the list "into bindings." Reword to "into the lock grant env".

## Round-4 verification table

| Round-4 finding | Round-5 disposition | Verification |
|---|---|---|
| B-1 `always_allow_session` grant creation not mechanically wired | Resolved | Re-emit only calls `mark_session_grant_requested` and rewrites reply to `allow`; gate receives the flag from `try_resolve`, creates the `UserGrant`, audits, and dispatches. New M-1 only asks for MarkError race handling. |
| M-1 provider detection cites nonexistent supervisor plan store | Resolved | §OP2 now uses live `Broker::plugin_acl(&canonical).and_then(|a| a.provider_id).is_some()` and removes the `managed` plan-store claim. |
| M-2 OpenRPC validation/fixture overclaim | Resolved | §OP2 assigns method-vs-tool consistency to `ToolSchemaCatalog::build`, and §TP3 adds the `send-mail` `openrpc.json` snippet plus tests. |
| M-3 `allow_secrets` through wrong effective-grant function | Mostly resolved → M-3 | §OP6 now targets `compile.rs::effective_grant`, not `sinks::union_bundle`; only the all-bundles vs default+named wording remains. |
| M-4 `allow_secrets` validation/warning contradictions | Mostly resolved → M-2 | Unused names are accepted by `validate::lock` and warned by `rfl install`; reserved-name validation is explicit. §Tr1 still needs the warning step. |
| M-5 session-grant classification unresolved | Resolved | CG4 uses the `session_grant_requested` flag from `try_resolve`; no payload extension or millisecond race remains. |
| M-6 missing audit kinds | Resolved | §AL1 lists `confirm_malformed` and `confirm_resolved_after_timeout`; §AL4 names tests. |
| N-1 CT0 stale methods | Mostly resolved → N-3 | Most method names are corrected; one `resolve` reference remains. |
| N-2 OP6 editorial aside | Resolved | The "actually wait" aside is gone. |
| N-3 A11 withdrawn lock path | Resolved | A11 now names `grant.bundles.<bundle>.env.allow_secrets`. |
| N-4 OP2 numbering | Resolved | OP2 renumbered the dead-code paragraph to 9. |

## Convergence call

Blocking count: **0**. Major count: **3**. Nit count: **5**.

I found **zero blockers**, but I do **not** yet consider this round ready for owner ratification because the three majors are all in commit-plan-critical instructions. Expected remaining work: one short polish round. After CT5's MarkError path, Tr1's unused-`allow_secrets` warning step, and the all-bundles wording are folded, I expect this scope to be ready for owner ratification.

Owner-judgment items still worth surfacing:

1. m5a/m5b split: m5 is not closed until m5b ships the verbatim-exfil negative.
2. `grant_match` JSON-Schema-as-template-shape-contract: `/grant`-time validation, runtime structural-subset matching.
3. `env.allow_secrets`: additive manifest/lock schema and scrubber-signature change, with warnings for unused deployment-flexibility names at `rfl install` time.
