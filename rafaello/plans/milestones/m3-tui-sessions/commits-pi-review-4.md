# commits.md round-4 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `f5684eb`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 4 addresses the explicit round-3 findings, but the new c08 constructor text introduces a fresh per-commit greenability blocker: c08 now references `RenderNode` before c09 owns it. There is also still an unresolved dependency-story conflict around keeping the `rafaello` CLI crate free of `serde_json` while c08 constructors require `serde_json::Value`.

## Blockers

### B1 — c08 is not greenable: `tool_result` payloads/constructor reference `RenderNode`, but `RenderNode` is introduced only in c09

- **Where:** c08 lines 287-305; c09 lines 327-335.
- **Problem:** c08 now owns all eight payload structs and all `Entry::new_*` constructors. Both the `tool_result` payload from scope §E3 and the c08 constructor `Entry::new_tool_result(..., content: RenderNode)` require `RenderNode`. But c09 is the commit that introduces `RenderNode` and explicitly depends on c08.
- **Why it blocks:** A c08 implementer cannot compile the payload module or constructor surface without either inventing `RenderNode` early (stealing c09 ownership) or omitting the c08 API/tests. This violates the per-commit-green rule.
- **Fix:** Pick one ownership model before handing to implementers: either move `RenderNode`/`RawFormat` into c08, or move the `tool_result` payload/constructor and any constructor smoke coverage requiring `RenderNode` into c09 and make downstream commits depend on that owner. A third option is to merge c08+c09 and renumber, but the current split is internally impossible.

### B2 — c31 still claims no direct `serde_json` dependency, but c08 constructors expose `serde_json::Value`

- **Where:** c08 lines 292-315; c31 lines 921-925.
- **Problem:** c31 says the harness uses c08 constructors to keep the `rafaello` bin free of direct `serde_json` / `ulid` / `chrono` deps. But c08's constructor list requires `serde_json::Value` for `new_tool_call` and `new_unknown`, and c08 even says the harness uses `serde_json::json!` at its boundary.
- **Why it blocks:** The demo harness must create eight built-in entries plus one unknown. At minimum `tool_call` args and the unknown payload need JSON values. No commit adds `serde_json` to `crates/rafaello/Cargo.toml`, and c31 explicitly forbids that dependency. An implementer must either violate c31 or fail to compile the harness.
- **Fix:** Either (a) change the constructors needed by the harness so they do not require caller-side `serde_json::Value` (for example `new_tool_call_empty(...)`, `new_unknown_text(...)`, or similar core-owned helpers), (b) explicitly add `serde_json` to the `rafaello` crate in the owning commit and remove the no-direct-dep claim, or (c) re-export a core helper/API that lets the harness build the required demo entries without naming `serde_json`.

## High-priority findings

### H1 — c08 constructor contract is under-specified for tests and later harness use

- **Where:** c08 lines 301-324.
- **Problem:** The constructor list references `ToolCallStatus`, but c08's type list does not explicitly introduce it. The `tool_call` payload shape requires an `id`, but `Entry::new_tool_call(name, args, status)` has no `id` parameter and does not say whether it generates one, derives it from the entry ULID, or uses some other value. The acceptance also says to assert `metadata.author` "per kind" but gives no author mapping.
- **Risk:** Even after B1/B2 are fixed, different c08 implementers can produce incompatible payloads and smoke tests. The c31 harness may also need a stable tool-call id to create a matching `tool_result`.
- **Fix:** In c08, explicitly introduce `ToolCallStatus`, specify the tool-call id policy/signature, and list the expected author for each constructor.

### H2 — c21 bus-publish wiring test is contradictory and may not observe the claimed error

- **Where:** c21 lines 674-685.
- **Problem:** The new test says the fixture publishes "on a topic in the `tui` ACL's grant" but immediately says the m3 grant is empty and expects `PublishOutsideGrant`. It also says the fixture uses `peer.notify("bus.publish", ...)`; notification errors are not returned to the child, so the row does not say how the test deterministically observes `BrokerError::PublishOutsideGrant`.
- **Risk:** The intended wiring assertion is good, but the test as written leaves implementers to invent a fixture mode/synchronization and error-observation mechanism. A missing or miswired service could still be hard to distinguish from a swallowed notification error unless the observation path is specified.
- **Fix:** Name the exact fixture mode/path (for example an existing `publish_full_params` mode if that is intended), say the topic is deliberately **outside** the empty grant, and specify the assertion mechanism, e.g. observe the broker's `core.lifecycle.publish_rejected` event with code `publish_outside_grant` from a registered observer.

## Medium-priority findings

### M1 — c20's dependency line hides the actual `RegisteredFrontend` owner

- **Where:** c20 lines 583-601.
- **Problem:** `shutdown_with_outcome` takes `Option<RegisteredFrontend>`, but `RegisteredFrontend` is introduced in c15. c20 lists only c18 and describes c18 as providing `RegisteredFrontend`.
- **Fix:** Per the file's own dependency convention, add c15 to c20's `Depends on` line, or remove `RegisteredFrontend` from the c20 signature. The current ordering is transitive-green, but the dependency row is inaccurate.

### M2 — c21 says "Eight tests" but lists nine

- **Where:** c21 lines 656-685.
- **Problem:** The acceptance count was not updated when `frontend_bus_publish_service_routes_to_handle_frontend_publish.rs` was added.
- **Fix:** Change "Eight tests" to "Nine tests".

### M3 — c28 still points manual validation at c32

- **Where:** c28 lines 849-853; Phase 4 section lines 965-974.
- **Problem:** c28 says a manual smoke recording lands in c32 `manual-validation.md`, but c32 is now automated-only and `manual-validation.md` is explicitly driver-owned Phase 4.
- **Fix:** Replace the c28 sentence with a Phase-4 reference, not c32 ownership.

### M4 — stale changelog bullets reduce trust in the plan metadata

- **Where:** "What changed from prior drafts" lines 1017-1023.
- **Problem:** The changelog says c19+c20 now declare c15 for `try_reserve_frontend_registration`, but c20 neither declares c15 nor owns `try_reserve_frontend_registration`. It also says c30/c31 declare c17/c25 for fixture modes + TUI test mode, while the current c30 row correctly depends on c26 for test-mode sentinels and c31 does not declare c25.
- **Fix:** Update or delete those stale bullets so the historical summary matches the actual rows.

## Summary

Round 4 is closer, but not ratifiable. Fix the c08/c09 ownership cycle and the c31/`serde_json` dependency contradiction first; those are execution blockers. Then tighten the c08 constructor details, make the c21 wiring test observable, and clean up the remaining count/dependency/stale-metadata drift.
