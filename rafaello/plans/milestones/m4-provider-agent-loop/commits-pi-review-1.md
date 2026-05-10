# m4 commits.md round-1 pi review

> Verdict: blocking
> Counts: b/7 h/4 m/3 l/2

## Coverage

| scope.md section | commit(s) | review |
|---|---:|---|
| W1-W5 | c01-c04, c02 | Covered. |
| B0 | c07, c10, c12, c18 | **Gap:** plugin/frontend inbound `request_id` enforcement is not assigned. |
| B1-B3 | c07 | Covered by explicit workspace cutover waiver. |
| B4 | c08 | Covered. |
| B5 | c09 | Covered. |
| B6/B7b | c10 | Covered in intent, but c10's positive internal-subscriber test is not implementable before c11. |
| B7 | c11 | Covered. |
| B8/B10 | c12 | Covered. |
| B11 | c13 | Covered. |
| F | c15 | Grant covered; re-emit test is misplaced. |
| PS + M2 | c14 | Covered; provider fixture publish is under-specified. |
| CR | c17-c18 | Main directions covered; `unknown_tool` and `reemit_rejected` behaviours lack tests. |
| AL + TD | c19 | Covered in intent, but `Capabilities` API gap blocks implementation. |
| PR | c20-c21 | **Gap:** fixture manifest compile-test lacks package `bin/` entry file. |
| TP | c22-c23 | **Gap:** same fixture-entry issue as PR. |
| M1 | c05-c06 | Covered; c05 acceptance has non-concrete either/or wording. |
| H6 | c14 / no-op | Covered by explicit no-new-hooks statement. |
| T | c16 | Covered. |
| C | c24-c26 | Load/spawn/wire-up covered; c26 is untested in-commit and contradicts lock-required path. |
| I | distributed | **Gap:** scope names `broker_plugin_tool_result_missing_in_reply_to_rejected.rs` but no commit lands it. |
| H | c21, c23, test common modules | Implicitly covered; harness placement should be made explicit. |

## Blockers

- **B1 — c20/c22 manifest compile-tests will fail against the live package validator.** c20/c22 create fixture manifests with `entry = "bin/rfl-mockprovider"` / `entry = "bin/rfl-readfile"` and immediately validate via `manifest::validate_with_package` (`commits.md:1229`, `commits.md:1255`, `commits.md:1319`). The live validator checks that the entry path exists inside the package (`validate_with_package.rs:18`, `validate_with_package.rs:29`-`34`). No commit creates `fixtures/.../bin/rfl-*` before those tests. Add executable fixture entry shims/symlinks in c20/c22 or move compile-tests after installing/copying the bins.

- **B2 — c10's positive provider publish test depends on c11 while c10 declares a no-op.** c10 says `notify_internal_subscribers` is a placeholder no-op until c11 (`commits.md:539`) but its acceptance requires `broker_publish_provider_topic_to_internal_subscriber.rs` to observe a mock receiver (`commits.md:548`-`557`). That cannot be green without either a synthetic test seam or the real `subscribe_internal` from c11 (`commits.md:604`-`636`). Move the final positive to c11, or land the real internal subscriber before this test.

- **B3 — §B0 `request_id` enforcement is incomplete.** scope.md requires `MissingRequestId` for `.tool_request`, `.tool_result`, `.assistant_message`, and `.user_message` in every handler (`scope.md:957`-`969`). commits.md only assigns provider enforcement in c10 (`commits.md:512`-`516`) and core enforcement in c12 (`commits.md:703`-`707`); c18 merely assumes frontend/plugin enforcement already exists (`commits.md:1057`, `commits.md:1070`-`1071`). The scope-named `rafaello-core/tests/broker_plugin_tool_result_missing_in_reply_to_rejected.rs` also lands nowhere (`scope.md:2830`). Add explicit `handle_plugin_publish` and `handle_frontend_publish` `request_id` enforcement + tests in the same commits.

- **B4 — c15 acceptance orders a re-emit-named test before re-emit exists.** c15 accepts `frontend_publish_user_message_reemitted_as_core_session_user_message.rs` while saying re-emission is c18 territory and the c15 test only checks grant acceptance (`commits.md:908`-`915`). This violates dependency order and gives a fresh agent a misleading test name. Rename the c15 test to a grant-only test, and land the scope-named re-emit test in c18.

- **B5 — c19 `AgentLoop` API omits `Capabilities` but calls `finalize_entry(..., &caps)`.** The c19 struct/constructor include broker/ACL/controller/shutdown only (`commits.md:1120`-`1133`), then AL3 calls `controller.finalize_entry(..., &caps)` (`commits.md:1142`). The live API requires a `&Capabilities` argument (`session/mod.rs:275`), and current `rfl chat` creates `Capabilities::tui_default()` outside the controller (`lib.rs:166`). Add `Capabilities` (or a render policy) to `AgentLoop::new` and c26 wiring, or change the controller API before c19.

- **B6 — c26 is a major code commit with no same-commit test.** c26 wires ReemitRouter + AgentLoop + TUI startup + wait loop + shutdown (`commits.md:1487`-`1514`) and explicitly says no dedicated test until c27 (`commits.md:1523`-`1524`). That fails the plan bar: code and tests must land together. Add a small c26 orchestration smoke test, or intentionally combine c26+c27 with a size waiver.

- **B7 — c26 acceptance contradicts the c24 lock-required path.** c24 adds `rfl_chat_missing_lock_errors.rs` and `LockNotFound` for no `rafaello.lock` (`commits.md:1438`-`1440`; `scope.md:1597`-`1598`). c26 then says existing m3 `rfl chat` tests still pass because the no-plugin-tree path is unchanged (`commits.md:1516`-`1519`). It is not unchanged after c24. Update/replace the m3 tests with lock-backed fixtures, or preserve an explicit no-lock compatibility mode.

## High

- **H1 — CR error paths are code without tests.** c17 promises `core.lifecycle.reemit_rejected` on re-emit failure (`commits.md:1007`), and c18 promises `core.lifecycle.tool_dispatch_rejected` for unknown tools (`commits.md:1031`-`1032`), but the c18 eight-test list covers only happy directions + taint/missing-in-reply-to (`commits.md:1077`-`1107`). Add tests for both lifecycle events.

- **H2 — c14 provider fixture publish is under-specified after B0/B6.** c14 adds `provider_bus_publish` publishing a synthetic `provider.<RFL_PROVIDER_ID>.tool_request` (`commits.md:879`-`880`), but c10 requires `request_id` and `in_reply_to` on provider tool_request (`commits.md:512`-`524`). Spell out `request_id: Some(...)` and `in_reply_to: []` in c14 acceptance.

- **H3 — c10 exceeds the stated commit sizing despite a partial waiver.** c10 is already marked ~250 LoC and bundles handler, two observed-id maps, topic-class stale rules, taint stripping, placeholder internal dispatch, and ~14 tests (`commits.md:481`-`566`). Split maps/bookkeeping from handler dispatch if possible; if not, expand the waiver to cover the test bulk and c11 dependency explicitly.

- **H4 — `ToolSpawnFailed` has no negative.** c24 adds `ToolSpawnFailed` to `RflChatError` (`commits.md:1421`-`1425`) and c25 emits it on eager tool spawn failure (`commits.md:1468`-`1472`), but acceptance only tests provider spawn failure (`commits.md:1481`-`1484`). Add `rfl_chat_tool_spawn_failure_propagates.rs` or remove the promised tested surface.

## Medium

- **M1 — c05 acceptance is not concrete.** It allows either a new `env_scrubber_rejects_rfl_provider_id.rs` or extending `env_scrubber_reserved_m2_names.rs`, with the driver deciding later (`commits.md:238`-`242`). Pick one exact file in the row.

- **M2 — active-provider pattern wording is ambiguous.** c17 stores `active_provider: CanonicalId` but subscribes to `provider.<active-provider-id>.**` (`commits.md:977`-`993`). The topic segment is the public `provider_id`, not the canonical id. Say explicitly that the router looks up `acl.plugins[active_provider].provider_id` and subscribes to `provider.<provider_id>.**`.

- **M3 — harness additions are implicit.** scope.md names `MockProviderHandle`, `ReadFileToolHandle`, and common taint/reemit asserts (`scope.md:2929`-`2950`), but commits.md only says harness extensions land alongside tests (`commits.md:1620`-`1623`). Add explicit bullets to c21/c23/c18 so per-commit agents know where to create/extend common modules.

## Low

- **L1 — c01 calls itself members-list-only while creating crate placeholders.** The body says c01 is “members-list-only” and then creates two `Cargo.toml` + `src/lib.rs` placeholders (`commits.md:129`-`148`). This is probably necessary for green builds, but reword it as “workspace-member placeholder cutover”.

- **L2 — avoid optional test collapsing language.** c10 says the driver may collapse one-file-per-case negatives if budget allows (`commits.md:559`). For pi review, keep the exact expected filenames or explicitly list acceptable grouped filenames.

## Notes

- Prior m1/m2/m3 commit plans use the same broad structure (row-per-idea, acceptance with named tests), but they also show that no-test code commits are rare and explicitly justified. c26 needs that treatment or a same-commit test.
- The 28-commit total is in the scope budget; the blocking issues are dependency/test/API correctness, not raw count.
