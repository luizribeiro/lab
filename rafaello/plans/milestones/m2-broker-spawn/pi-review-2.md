# Pi review round 2 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial round 2. Round 1's largest category of API-coordinate mistakes was improved substantially, but the rewrite introduced several new contradictions and left some executable-shape problems unresolved.

## Verdict

**Not ratifiable yet.**

The document is much closer to implementable than round 1: the fixture is now correctly inside `rafaello-core`, fittings transport is named, socketpair identity is preserved, and the broker namespace algorithm is mostly structurally correct. However, the current draft still contains build breakers, impossible lockin API usage, contradictory public signatures, and test cases whose behavior is deliberately postponed to `commits.md`. Those are exactly the things a scope must settle before commit planning.

## Blocking findings

### 1. `Broker::new` has two incompatible signatures, and the boot-event contract is still undefined

Sections: B1, B9, B10, integration tests.

- B1 declares `Broker::new(acl: BrokerAcl) -> Self` (`scope.md:225`).
- B10 says construction can fail and must return `Result<Self, BrokerError>` with `InvalidTopic` / `InvalidPattern` (`scope.md:476-482`).
- The test matrix uses both unresolved models: `Broker::new(acl)` plus an undefined `init_post_register` API or an undefined "first subsequent register emits boot" behavior (`scope.md:1024`).

This cannot be left to `commits.md`. Pick one API and one boot behavior now. If `Broker::new` validates ACLs, it must return `Result`; B1, H3, tests, and acceptance should all use `Broker::new(acl)?`. If boot is observable, define the actual API/event timing; emitting at construction is a no-op before any subscriber exists.

### 2. `BrokerError` is missing variants that later sections require, and one variant cannot represent core misuse

Sections: B2, B10, core publish tests.

B10 requires `InvalidPattern` (`scope.md:481-482`), but B2 does not define it (`scope.md:260-292`). The core-publish negative test expects `publish_core("plugin.x.y", ...) -> BrokerError::PublishOnReservedNamespace` (`scope.md:1027`), but the scoped variant is `PublishOnReservedNamespace { canonical: CanonicalId, topic: String }` (`scope.md:271-274`), which cannot represent a core publisher without inventing a fake canonical.

Either make the publisher field optional/typed (`PublisherIdentity`/`Option<CanonicalId>`) for all relevant errors, or add a distinct `CorePublishOnReservedNamespace` variant. As written, the error surface cannot compile against the stated tests.

### 3. W1 still omits required dependencies introduced by the rewrite

Sections: W1/W2, B1, SP3.

The scope explicitly uses `parking_lot::Mutex` twice (`scope.md:223`, `scope.md:535`) but W1 does not add `parking_lot`. `SpawnError::Lockin` publicly names `anyhow::Error` (`scope.md:596`, `scope.md:614-615`) but W1 does not add `anyhow` to `rafaello-core`. These are direct build breakers, not implementation details.

### 4. `BrokerError: Clone + PartialEq` is false against the actual m1 code

Sections: B2/E.

B2 says `BrokerError` derives `Debug + Clone + PartialEq` and claims `ValidationError` is already `Clone + PartialEq` (`scope.md:293-297`; repeated at `scope.md:1136-1138`). In the current repo, `rafaello_core::error::ValidationError` derives only `Debug, Error`; it is not `Clone` or `PartialEq`.

Options: extend `ValidationError` derives in m2, store a cloneable/comparable summary in `BrokerError::InvalidTopic`, or drop `Clone + PartialEq` and make tests use `matches!`. The draft currently asserts a repository fact that is not true.

### 5. The env-clear/private-tmp plan uses a lockin API that does not exist

Sections: SP4 step 12, lockin inputs.

The scope calls `builder.command(...) -> SandboxedCommand`, then says:

```rust
cmd.env_clear();
cmd.env("TMPDIR", sandbox.private_tmp());
```

and adds that `SandboxedCommand` exposes the private tmp via the sandbox handle (`scope.md:702-712`). Current `lockin::SandboxedCommand` owns the `Sandbox`, but does **not** expose `private_tmp()` or a sandbox handle before spawn. `SandboxBuilder::build` is crate-private. The Tokio wrapper has the same limitation.

This means the scoped implementation cannot both call `env_clear()` and restore lockin's private tmp with current lockin. The scope must either add a small lockin API in scope, avoid `env_clear`, or accept losing the private tmp env guarantee. "Verify at commits.md time" is not acceptable for a known public-API mismatch.

### 6. `RFL_TOPIC_ID` is required by the fixture but never actually injected or rejected in SP4

Sections: SP4, F2, F5.

F2's bad-grammar mode depends on `RFL_TOPIC_ID` supplied by the supervisor (`scope.md:927-931`). F5 says `RFL_TOPIC_ID` joins the reserved env set (`scope.md:989-1003`). But SP4 step 4's reserved list omits it (`scope.md:637-640`), and SP4 step 12 injects only `RFL_BUS_FD`, `RFL_PLUGIN`, `RFL_PROJECT_ROOT`, and `RFL_PRIVATE_STATE_DIR` (`scope.md:729-733`).

This breaks multiple fixture modes and leaves a collision hole for the exact reserved var F5 introduces.

### 7. The fixture-side `Client::connect(transport)` call is not a real fittings API

Section: F3.

F3 says to build a `StdioTransport` and then call `Client::connect(transport)` (`scope.md:977-982`). In the landed fittings API, `Client::connect` takes a `Connector`, not a raw `Transport`. There is no `Client::from_transport` in the current client crate.

The scope hints at "otherwise Connector impl wrapping the prepared transport", but does not actually specify that wrapper. Since the fixture binary is part of the acceptance harness, this needs to be concrete: define the local one-shot connector type and its ownership semantics, or add a fittings API as an in-scope prerequisite.

### 8. Post-spawn transport setup errors have no error variant and no cleanup path

Sections: SP3, SP4 steps 13-17.

SP4 spawns the child at step 13, then converts `core_fd` to a Tokio `UnixStream` at step 14 (`scope.md:739-749`). `set_nonblocking` and `UnixStream::from_std` can return `std::io::Error`. SP3 has no transport-build/io-after-spawn variant other than `Spawn` (already used for `cmd.spawn`) and `FittingsBuild { source: FittingsError }` (`scope.md:596-600`), which does not fit these errors.

The unwind rules only mention `broker.register_plugin` failure at step 17 (`scope.md:759-763`). The draft must define how steps 14-16 errors map and that child/proxy/fds are cleaned up for every failure after step 13.

### 9. The `RegisteredPlugin` `!Send` requirement poisons the supervisor handle design

Sections: B1, SP1/SP5.

B1 requires `RegisteredPlugin` to be `!Send` (`scope.md:229-231`). The supervisor then stores registration guards inside cloneable `SpawnHandle`/registry state whose drops may occur on a multi-thread Tokio runtime, and the test matrix says most tests are `#[tokio::test(flavor = "multi_thread")]` (`scope.md:1007-1008`). A `!Send` guard inside the shared spawn state will make `SpawnHandle`/`PluginSupervisor` awkward or non-`Send`, and can block use from spawned tasks or multi-thread futures.

If the intent is only `!Clone`, say that. The guard should likely be `Send` if it is just an `Arc<BrokerInner>` plus canonical id and optional one-shot state. Do not bake `!Send` into the public contract unless the rest of the supervisor API is explicitly single-threaded.

### 10. `SpawnHandle::wait`/`try_wait` are underspecified after moving child ownership into the reaper

Sections: SP1.

The reaper task owns the `SandboxedChild` (`scope.md:543-550`, `scope.md:764-769`). But the public handle also exposes `try_wait(&self) -> io::Result<Option<ExitStatus>>` (`scope.md:521-522`). Once the child is moved into the reaper, `try_wait` cannot call the child. It can only inspect cached reaper state, which is a different semantic from `std::process::Child::try_wait`.

Define the actual semantics: cached-observation only, maybe no `io::Error`; or keep child ownership behind a mutex and have the reaper/handle coordinate. The current text promises both models at once.

### 11. Provider refusal is specified with non-existent broker API and an omitted enum variant

Sections: SP3/SP4, retrospective.

SP4 step 7 calls `broker.acl_entry_grants_publish_or_subscribe_or_provider_only(...)` (`scope.md:650`), which is not in the scoped public broker surface. It also returns `InvalidPlanReason::ProviderNotInM2` (`scope.md:654-655`), but SP3's `InvalidPlanReason` enum does not include that variant (`scope.md:604-609`).

The provider staging decision may be fine, but the scope must specify a real way to detect it (e.g. ACL/plugin `provider_id.is_some()` or `CompiledPlugin.provider_id.is_some()`) and add the missing variant.

### 12. Duplicate spawn is checked only after launching the second child

Sections: B1, SP4.

B1 can detect `AlreadyRegistered` (`scope.md:226-229`), but SP4 does not perform a live-registration precheck in Phase A. The duplicate is discovered at step 17, after socketpair, proxy, sandbox command, and child spawn (`scope.md:660-763`). That is unnecessary side effect for a cheap validation error and makes `supervisor_spawn_duplicate_canonical_refused` depend on best-effort teardown of a child that should never have been launched.

Add an explicit `broker.is_registered`/`try_reserve_registration` phase before resource allocation, or intentionally document the side effect as accepted. The current "all cheap validation runs first" claim (`scope.md:622-624`) is not true for duplicates.

### 13. Private-state/project-root inference from `write_dirs` is too implicit for a runtime primitive

Section: SP4 step 11.

The supervisor computes private state by scanning `plan.filesystem.write_dirs` for a segment pattern (`scope.md:691-701`). That works only because m1 currently injects one conventional path, but the plan type does not carry `project_root` or `private_state_dir`, and public-field mutation or a legitimate extra write dir with the same suffix can make the inference ambiguous.

For a locked spawn primitive, prefer an explicit field in `CompiledPlugin` or a deterministic helper from m1 that returns the path. If keeping inference, specify exact matching and ambiguity errors.

### 14. Manual validation contains an invalid Cargo command

Section: manual validation.

The command at `scope.md:1085-1087` puts `--test supervisor_spawn_fixture_happy_path` after Cargo's `--`, so it is passed to the test binary, not Cargo. It should be shaped like:

```sh
cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core \
  --features test-fixture --test supervisor_spawn_fixture_happy_path -- --nocapture
```

This is small, but manual validation commands are acceptance criteria; they must be copy-pasteable.

## Non-blocking but should fix before ratification

1. **Outpost signature text is stale.** Inputs say `outpost_proxy::start(policy) -> io::Result<ProxyHandle>` while SP4 correctly uses `.await`. Spell it as an async function to avoid another round of coordinate drift.
2. **`BrokerError::Internal` mentions lock poisoning while the broker uses `parking_lot`, which does not poison.** Either keep the variant generic or remove that example.
3. **`core.lifecycle.publish_rejected` says "after every rejection" but its list omits `InvalidPayload` while the code list includes `invalid_payload`.** Align prose and code list.
4. **The fixture feature risk is real.** The scope says tests fail to compile without `--features test-fixture`; if that remains true, every per-commit prompt must include the feature or agents will report false failures.

## Summary of required edits

Before ratification, the scope needs at least these concrete changes:

- Normalize `Broker::new` to `Result` or non-`Result` everywhere; define boot delivery.
- Complete `BrokerError` (`InvalidPattern`, core publish misuse shape) and reconcile derives with actual `ValidationError`.
- Add missing deps (`parking_lot`, `anyhow`) or remove their use.
- Fix lockin private tmp access or change the env-clear strategy.
- Inject and reserve `RFL_TOPIC_ID` consistently.
- Specify the fixture `Connector` wrapper for fittings client.
- Define cleanup/error mapping for every post-spawn failure.
- Remove or justify `RegisteredPlugin: !Send`.
- Add `ProviderNotInM2` and a real provider-detection path.
- Make duplicate registration fail before spawning.
- Fix manual validation command syntax.
