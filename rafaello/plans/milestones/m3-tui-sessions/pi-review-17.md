# pi review 17 — m3 scope round-17 adversarial review

Reviewed: `plans/milestones/m3-tui-sessions/scope.md` at commit `7124ac0` (3304 lines).

Verdict: **no blockers found**. The round-17 draft is much closer to executable than prior rounds, and the remaining issues are scope-text/API-ordering fixes rather than architectural blockers. I would revise the draft before implementation so cleanup/error-path and hook-placement details do not mislead commit planning or tests.

Counts: **0 blocker / 2 high / 2 medium / 1 low**.

## High

### H1. Post-spawn `rfl chat` errors can bypass the explicit shutdown + stderr-drain contract

`scope.md:2077-2090` runs replay and the in-test fixture harness after the TUI has been spawned and after the stderr forwarder has been started. Both paths contain fallible operations:

- `controller.replay_history(&caps).await?`;
- repeated `controller.finalize_entry(entry, &caps).await?` calls;
- the final `broker.publish_core("core.lifecycle.test_done", ...)` harness publish.

The canonical cleanup is only specified in the step-7 readiness error arms and in step 10 after `frontend_handle.wait().await` (`scope.md:2092-2260`): call `frontend_handle.shutdown().await`, then await/drain the stderr forwarder so the combined-stream tests are deterministic.

If replay/finalize/test-done publish fails, the current flow can return from `run_chat` before step 10 and skip that explicit shutdown/drain. `FrontendHandle::Drop` is intentionally best-effort and non-blocking; it does not await the serve loop, does not produce the `ShutdownReport`, and does not drain the line-forwarding task. That contradicts the milestone's repeated guarantees that post-spawn error paths use bounded shutdown and flush forwarded TUI stderr before exit.

Recommended fix: specify a post-spawn cleanup guard or an explicit `match`/`scopeguard` pattern around steps 8-10. Once step 6 succeeds, every later error path should funnel through exactly one bounded `frontend_handle.shutdown().await` plus `stderr_forwarder.await` before returning the `RflChatError`.

### H2. H6 post-register inject point is still ambiguous against current m2 spawn ordering

`scope.md:2338-2353` defines the H6 post-register fault as occurring after `broker.register_plugin(...)` succeeds and before the function returns `Ok(handle)`, with the parenthetical "between register and `Server::serve` install". The same paragraph says the child is spawned and "the reaper is already running". The restored `supervisor_spawn_post_register_reaps_child.rs` test depends on that latter property.

In the current m2 `PluginSupervisor::spawn` implementation, the order is:

1. build server and peer;
2. `broker.register_plugin(...)`;
3. drop the in-flight guard;
4. create the watch channel and spawn the reaper/watcher tasks;
5. spawn `server.serve()`;
6. build and store the managed record, then return.

Therefore, "after register, before serve install" is a broad window that includes a sub-window where the reaper is **not** yet running. Injecting there would force direct `Child` cleanup, not the reaper-driven cleanup asserted by `post_register_reaps_child`.

The final sentence also says round 17 does not inject between `tokio_command.spawn()` and `register_plugin` because "the reaper-task ownership transition happens in that window" (`scope.md:2348-2351`). In the current code, that transition happens after registration, not before it.

Recommended fix: pin the hook in code-order terms, e.g. "after the reaper/watcher tasks have been spawned and before `tokio::spawn(server.serve())`". Also correct the spawn-to-register-window explanation so implementers do not place the hook before the ownership transition.

## Medium

### M1. Frontend private-state directory creation is required but missing from the spawn algorithm

`scope.md:853-858` says `FrontendSupervisor::spawn` injects `RFL_PRIVATE_STATE_DIR` pointing at `${PROJECT_ROOT}/.rafaello-frontend-data/<attach-id>/`, and `scope.md:1517-1522` says that directory is created by `FrontendSupervisor::spawn` if missing.

However, §F3's Phase B sequence never includes a `create_dir_all` step, its ordering, or its error mapping. This is observable implementation guidance, not just prose: creating the directory before child spawn needs no child unwind; creating it after child spawn requires killing/reaping on failure. The plan should not leave that choice implicit.

Recommended fix: add an explicit pre-spawn Phase B step after attach-id validation/path derivation and before `Command::spawn()`:

- derive the private state dir from `paths.project_root` and the validated `AttachId`;
- `fs::create_dir_all(&private_state_dir)`;
- map failure to `FrontendSpawnError::Io` or a dedicated `PrivateStateDirCreate { attach_id, path, source }`;
- only inject `RFL_PRIVATE_STATE_DIR` after the directory exists.

### M2. `RFL_PROJECT_ROOT` is specified as absolute, but CLI project-root resolution does not guarantee that

`scope.md:675-676` says `FrontendPaths.project_root` is passed to the child as `RFL_PROJECT_ROOT`. `scope.md:1794` says `rfl-tui` requires `RFL_PROJECT_ROOT` to be an absolute path. But `scope.md:1921-1922` only says `rfl chat` resolves the project root from cwd or `--project-root`; it does not say the override is canonicalized or absolutized.

A user or test can pass a relative `--project-root`, causing the parent to inject a relative `RFL_PROJECT_ROOT` and the TUI to fail before readiness. That would surface as a frontend-startup failure even though the CLI could normalize it deterministically.

Recommended fix: specify a `resolve_project_root` helper that turns cwd/default and `--project-root` into an absolute path before constructing `FrontendPaths` and opening the session store. Either `canonicalize` existing paths or join relative overrides against cwd and then validate `is_absolute()`. Add a CLI unit/integration test for a relative project-root override if the helper is public.

## Low

### L1. B4 describes an under-2-segment namespace branch that grammar validation will reject first

`scope.md:1264-1265` says a frontend publish on `frontend` alone / under-2 segments maps to `PublishOnReservedNamespace`. But broker publish handling validates topic grammar before namespace classification, and `validate_topic` rejects fewer than two segments as `InvalidTopic`.

This is not a serious implementation risk, but it is a small mismatch in the negative-case contract. Tests following the current text could assert the wrong error variant.

Recommended fix: reword the B4 bullet to cover valid but wrong frontend namespaces such as `frontend.other.foo` or `frontend.foo` when the second segment does not equal the attach id. Leave `frontend` alone to the grammar-validation / `InvalidTopic` path.

## Notes checked during review

- `cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture --no-run` completed successfully against the current workspace before this review was saved.
- The M1 unknown-namespace patch is now correctly scoped as a minimal additive validation change: current code has `check_publish_topic` as a private helper in `crates/rafaello-core/src/validate/mod.rs`, with existing `PublishOnReservedNamespace`, `PublishOnFrontendNamespace`, and `ProviderNamespaceMismatch` variants. I did not count this as a finding because round 17's intended behavior matches the current split well enough.
- The internal split still lists frontend supervisor before session store, but §S6 can be implemented as a tiny path-derivation/create-dir contract inside the supervisor commit once the explicit Phase B mkdir step above is added. I did not count commit ordering as a separate finding.
