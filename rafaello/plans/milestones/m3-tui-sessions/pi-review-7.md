Reviewed `plans/milestones/m3-tui-sessions/scope.md` at `8554b45`.

## Blocking

1. **`CompiledFrontend.attach_id` type is inconsistent**
   - §F1 says `attach_id: String` (`scope.md:385`).
   - §C2 constructs `attach_id: AttachId::new("tui")` (`scope.md:1393`).
   - Broker APIs take `AttachId` (`scope.md:723-735`).
   - Fix by making `CompiledFrontend.attach_id: AttachId` everywhere, or keep `String` and explicitly convert/validate at spawn.

2. **`EnvPlan` construction does not match the existing m1 type**
   - §F1 says `env: EnvPlan` is “m1’s type, unchanged” (`scope.md:387`).
   - Current `EnvPlan` is `pass: Vec<String>, set: BTreeMap<String, String>` with no `clear`.
   - §C2 uses `pass: btreeset![...]` and `clear: vec![]` (`scope.md:1396-1413`).
   - This will not typecheck. Use `Vec<String>` and remove `clear`, or explicitly scope an `EnvPlan` shape change.

3. **`FrontendHandle::shutdown(self)` conflicts with unconditional `Drop` SIGKILL**
   - §F4 says `Drop` sends best-effort SIGKILL (`scope.md:607-612`).
   - `shutdown(self)` consumes the handle but does not specify disarming `Drop` (`scope.md:614-632`).
   - In Rust, `Drop` still runs after a consuming method returns, so a graceful shutdown can still send SIGKILL afterward, with PID-reuse risk.
   - Fix: specify `FrontendHandle` stores pid/serve/register fields in `Option`s and `shutdown(mut self)` takes/disarms them before return; Drop only acts on remaining live fields.

4. **`ReaperOutcome` usage is incompatible with current repo type**
   - §F4 expects `ReaperOutcome::Exited(0)` (`scope.md:645`).
   - §C2 expects `Exited(n)` / `Signaled(_)` (`scope.md:1504-1507`).
   - Current `ReaperOutcome::Exited(std::process::ExitStatus)` has no `Signaled` variant.
   - Fix either by reshaping `ReaperOutcome` in scope, or by specifying checks via `ExitStatus::{success, code}` and Unix `ExitStatusExt::signal()`.

5. **`FrontendConfig` is referenced but never defined**
   - `FrontendSupervisor::new(broker: Broker, config: FrontendConfig)` is specified at `scope.md:383`.
   - No fields/defaults are pinned elsewhere.
   - Spawn/shutdown needs at least grace durations and probably fittings limits. Add the public struct shape or reuse/rename existing `SupervisorConfig`.

## Medium

6. **`rfl_chat_replay_withheld_until_frontend_ready` can hang for the TUI self-timeout**
   - Test pre-seeds entries and runs with `RFL_TUI_TEST_MODE=1` + ready delay (`scope.md:1904-1910`).
   - It does not set `RFL_HARNESS_FIXTURES=1`, so no `core.lifecycle.test_done` is published.
   - Headless TUI exits only on `test_done` or `RFL_TUI_MAX_LIFETIME` default 60s.
   - Fix by setting `RFL_TUI_MAX_LIFETIME=1` or having the test terminate after observing the three replay events.

7. **Controller-test observer helper conflicts with “single tui ACL” helper**
   - `in_memory_broker_with_tui_acl()` creates only a `tui` frontend ACL (`scope.md:2004-2006`).
   - `record_subscriber()` is “plugin-shaped” and used by controller tests (`scope.md:2007-2009`), which need registration rights.
   - Either make the observer frontend-shaped, or add an observer plugin ACL in a separate helper.

8. **Test placement is inconsistent for a pure session-store test**
   - Placement rules put session-store tests under `rafaello-core/tests` (`scope.md:1687-1689`).
   - `session_store_lock_fd_not_inherited_by_child.rs` is listed under `rafaello/tests` (`scope.md:1893-1894`).
   - Move it to core unless it genuinely needs the `rfl` binary.

## Low

9. **`CompiledFrontend` ACL fields look like dead/duplicated source of truth**
   - `CompiledFrontend` carries `subscribe_patterns`, `auto_subscribes`, and `publish_topics` (`scope.md:388-390`), while `BrokerAcl` is built separately before spawn (`scope.md:1381-1384`).
   - §F3 does not say spawn validates plan ACL fields against broker ACL.
   - Clarify whether those fields are used to construct ACL, checked against ACL, or removed.
