# commits.md round-1 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `4457c07`  
Reviewer: pi  
Verdict: **not ratified yet**

The draft is structurally close, but several items violate the per-commit green-workspace invariant or create impossible acceptance criteria. Fix the blockers before handing commits to per-commit implementers.

## Blockers

### B1 — c01 / c03 workspace-member contradiction

- **Where:** c01, c03.
- **Problem:** c01 title and What say it registers `crates/rafaello-tui` in workspace members, but c01 acceptance says unsourced members break cargo and therefore c01 is deps-only with registration folded into c03. c03 then scaffolds the crate but does not clearly own the workspace `members` edit.
- **Why it blocks:** A per-commit agent following c01 literally can make the workspace red by adding a non-existent member, while an agent following the acceptance text may omit a change promised by the commit title.
- **Fix:** Make c01 explicitly deps-only. Make c03 explicitly create `crates/rafaello-tui` and add it to `[workspace].members` in the same commit.

### B2 — c06 introduces frontend shutdown API before frontend types exist

- **Where:** c06, c17-c21.
- **Problem:** c06 extracts `rafaello_core::frontend::shutdown::shutdown_with_outcome(...) -> ShutdownReport`, but the `frontend` module, `ShutdownReport`, `FrontendConfig`, and the surrounding frontend lifecycle types are introduced later in c17-c21.
- **Why it blocks:** c06 cannot compile as written unless it also introduces a hidden subset of the frontend module/types, contradicting c17's type-only ownership.
- **Fix:** Either move the shutdown seam into the frontend group, or have c06 explicitly introduce the minimal `frontend::shutdown` module and all types it returns/accepts, then make c17 depend on that scaffold.

### B3 — c10 adds a temporary test that will fail after c12

- **Where:** c10, c12, c13.
- **Problem:** c10 acceptance adds `renderer_registry_with_builtins_is_empty_for_now.rs`, asserting `RendererRegistry::with_builtins()` has zero entries. c12/c13 intentionally populate `with_builtins()`.
- **Why it blocks:** Unless c12 rewrites or removes the c10 test, the workspace becomes red after built-ins land.
- **Fix:** In c10 test `RendererRegistry::new()` is empty instead, or explicitly assign c12 to update the c10 test to expect the built-ins added by c12/c13.

### B4 — c21 tests require fixture mode from c25

- **Where:** c21, c25.
- **Problem:** c21 acceptance uses `rfl-bus-fixture` `signal_ready`, while c25 is the commit that adds `signal_ready`. The note says c21 uses a “pre-c25 stub mode”, but that stub is not specified as part of c21.
- **Why it blocks:** A per-commit agent implementing c21 cannot satisfy its acceptance tests against the repo state available at c21.
- **Fix:** Move c25 before c21, or make c21 explicitly add the minimal fixture behavior it uses and have c25 later extend/rename it. Prefer moving c25 earlier because several later tests also depend on the fixture additions.

### B5 — c25 `probe_fd_closed` errno is wrong

- **Where:** c25.
- **Problem:** c25 says `probe_fd_closed` calls `F_GETFD` and exits 0 on `ESRCH`. The scope says correctly that `F_GETFD` on a closed/non-inherited fd returns `EBADF`.
- **Why it blocks:** Implementing the commit text literally produces the wrong fixture behavior and failing fd-inheritance tests.
- **Fix:** Change c25 wording and acceptance to `EBADF`.

### B6 — c28 depends on a future commit

- **Where:** c28, c29.
- **Problem:** c28 says `Depends on. c27, c29`, but c29 lands after c28. The c28 text still says “reorder if pi prefers,” leaving ordering unresolved.
- **Why it blocks:** Sequential implementation cannot satisfy a dependency on a future commit.
- **Fix:** Swap c28/c29 so the painter lands before the production UI loop, or make c28 genuinely use only a local stub and remove the dependency on c29.

## High-priority findings

### H1 — frontend commit dependencies omit broker frontend registration prerequisites

- **Where:** c17, c18.
- **Problem:** c17 introduces frontend types that reference `RegisteredFrontend`-style lifecycle ownership and c18 calls `try_reserve_frontend_registration`, but c17/c18 only depend on c02/c14. The actual broker registration APIs and errors land in c15.
- **Risk:** Per-commit agents may either invent duplicate frontend registration scaffolding or fail to compile.
- **Fix:** Add c15 as a dependency for c17/c18 where the types or validation use `RegisteredFrontend`, `BrokerError` frontend variants, or `try_reserve_frontend_registration`.

### H2 — c29 likely depends on c17 for `PaintError`

- **Where:** c29, c17.
- **Problem:** c29 exposes `draw_with_panic_isolation(...) -> Result<(), PaintError>`, while `PaintError` is introduced in c17.
- **Risk:** c29 as written depends only on c09 and c03, so implementers may create a second `PaintError` in the TUI crate or fail to compile against the public error surface expected by scope.
- **Fix:** Add c17 as a dependency for c29, or explicitly move/define `PaintError` in the crate that owns painting and adjust c17 accordingly.

### H3 — c31 acceptance depends on fixture and TUI test-mode behavior landed elsewhere

- **Where:** c31, c25, c27.
- **Problem:** c31 tests include env override using real `rfl-tui` in test mode plus frontend exit/timeout cases. Those require c25 fixture modes (`exit_immediately`, `hold_silent`, etc.) and c27 TUI test-mode/self-timeout behavior.
- **Risk:** The c31 acceptance is under-specified unless those commits have landed and are declared dependencies.
- **Fix:** Add c25 and c27 dependencies to c31 where applicable, or split tests so c31 only covers orchestration that is actually available at that point.

### H4 — forward references to helpers/modes are not owned by concrete commits

- **Where:** c19, c24.
- **Problem:** c19 says its smoke test will be re-pointed after c25, and c24 references `in_memory_broker_with_tui_and_observer_acl()` from “c25's harness preview,” but c25 is only fixture-mode work and does not define that helper.
- **Risk:** These are impossible instructions for isolated per-commit agents because the agent prompt contains a future promise but no actionable owner.
- **Fix:** Remove the forward references, define the helpers in the same commit that first uses them, or add a dedicated earlier test-harness helper commit.

## Medium-priority findings

### M1 — c07 test-file count does not match the list

- **Where:** c07.
- **Problem:** The text says “five new test files” but lists five unwind bullets plus an additional Linux fd-baseline twin and then the dead-watch tests. Depending on interpretation, this is six or more files.
- **Fix:** Correct the count and split the bullet list into unwind tests vs shutdown seam tests.

### M2 — c11 test count does not match the list

- **Where:** c11.
- **Problem:** The acceptance says “Five new tests” but lists six test names.
- **Fix:** Change the count to six or remove/merge one listed test.

### M3 — c31 test count does not match the list

- **Where:** c31.
- **Problem:** The acceptance says “Three CLI tests” but lists seven tests.
- **Fix:** Change the count to seven, or split the listed tests across c31/c32.

### M4 — c06 H6 pre-spawn placement should mirror scope exactly

- **Where:** c06.
- **Problem:** c06 says pre-spawn is “post-socketpair-pre-spawn,” while scope H6.2 clarifies it is after socketpair/proxy/private-state-dir creation and before `tokio_command.spawn()`.
- **Risk:** Implementers may put the hook too early and fail to exercise the intended unwind resources.
- **Fix:** Copy the scope wording into c06, including that private-state-dir cleanup is not part of the unwind contract.

### M5 — group-count/header drift

- **Where:** review prompt / commits.md structure.
- **Problem:** The file is described as 34 commits in 11 groups, but the headings run Group 0 through Group 11, i.e. 12 groups.
- **Fix:** Either update the summary count or renumber/merge groups.

## Notes

- `--features rafaello-core/test-fixture` was checked against the current workspace and works with the current Cargo setup, so the workspace-qualified feature spelling is not itself a blocker.
- The largest structural recommendation is to add a “no forward references in acceptance” rule: every acceptance item for cN must rely only on code, helpers, fixture modes, and binaries landed by cN and its declared dependencies.
